use crate::config::Config;
use crate::model::Network;
use candle_core::{Device, Result, Tensor, D};
use candle_nn::{AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap, ops};
use rand::seq::SliceRandom;

pub fn execute(
    train_in: &Tensor,
    train_out: &Tensor,
    train_mask: &Tensor,
    valid_in: &Tensor,
    valid_out: &Tensor,
    valid_mask: &Tensor,
    map: &VarMap,
    device: &Device,
    config: &Config,
) -> Result<()> {
    let variables = VarBuilder::from_varmap(map, candle_core::DType::F32, device);
    let network = Network::new(
        variables,
        config.vocab,
        config.dim,
        config.heads,
        config.limit,
        config.drop,
        config.layers,
    )?;

    let mut parameters = ParamsAdamW::default();
    parameters.lr = 0.0003;
    let mut optimizer = AdamW::new(map.all_vars(), parameters)?;

    let (train_rows, train_cols) = train_in.dims2()?;
    let train_inputs = train_in.narrow(1, 0, train_cols - 1)?.contiguous()?;
    let train_targets = train_out.narrow(1, 1, train_cols - 1)?.contiguous()?;
    let train_mask = train_mask.narrow(1, 1, train_cols - 1)?.contiguous()?;

    let (_valid_rows, valid_cols) = valid_in.dims2()?;
    let valid_inputs = valid_in.narrow(1, 0, valid_cols - 1)?.contiguous()?;
    let valid_targets = valid_out.narrow(1, 1, valid_cols - 1)?.contiguous()?;
    let valid_mask = valid_mask.narrow(1, 1, valid_cols - 1)?.contiguous()?;

    let batch = 16;
    let mut gen = rand::rng();
    let mut indices: Vec<u32> = (0..train_rows as u32).collect();

    let mut best_loss = f32::MAX;
    let mut stale = 0u32;

    for epoch in 0..800 {
        indices.shuffle(&mut gen);
        let mut total = 0.0;
        let mut steps = 0;

        for start in (0..train_rows).step_by(batch) {
            let end = usize::min(start + batch, train_rows);
            let current = end - start;
            let chunk = &indices[start..end];

            let tensor_indices = Tensor::from_vec(chunk.to_vec(), current, device)?;
            let batch_inputs = train_inputs.index_select(&tensor_indices, 0)?;
            let batch_targets = train_targets
                .index_select(&tensor_indices, 0)?
                .flatten_all()?;
            let batch_mask = train_mask
                .index_select(&tensor_indices, 0)?
                .flatten_all()?;

            let predictions = network.forward(&batch_inputs, true)?;
            let reshaped = predictions.reshape(((), config.vocab))?;

            let log_probs = ops::log_softmax(&reshaped, D::Minus1)?;
            let gathered = log_probs.gather(&batch_targets.unsqueeze(1)?, 1)?;
            let nll = gathered.neg()?.squeeze(1)?;
            let masked = nll.broadcast_mul(&batch_mask)?;
            let valid = batch_mask.sum_all()?;
            let error = masked.sum_all()?.broadcast_div(&valid)?;

            optimizer.backward_step(&error)?;
            total += error.to_scalar::<f32>()?;
            steps += 1;
        }

        let average = total / steps as f32;

        let valid_preds = network.forward(&valid_inputs, false)?;
        let valid_reshaped = valid_preds.reshape(((), config.vocab))?;
        let valid_targets_flat = valid_targets.flatten_all()?;
        let valid_mask_flat = valid_mask.flatten_all()?;

        let valid_log_probs = ops::log_softmax(&valid_reshaped, D::Minus1)?;
        let valid_gathered = valid_log_probs.gather(&valid_targets_flat.unsqueeze(1)?, 1)?;
        let valid_nll = valid_gathered.neg()?.squeeze(1)?;
        let valid_masked = valid_nll.broadcast_mul(&valid_mask_flat)?;
        let valid_count = valid_mask_flat.sum_all()?;
        let valid_value = valid_masked
            .sum_all()?
            .broadcast_div(&valid_count)?
            .to_scalar::<f32>()?;

        println!("epoch {} train {} valid {}", epoch, average, valid_value);

        if valid_value < best_loss - 1e-4 {
            best_loss = valid_value;
            stale = 0;
            map.save("weights.safetensors")?;
        } else {
            stale += 1;
            if stale >= 80 {
                println!("early stop");
                break;
            }
        }
    }

    Ok(())
}