use crate::config::Config;
use crate::model::Network;
use candle_core::{Device, Result, Tensor};
use candle_nn::{loss, AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};
use rand::seq::SliceRandom;

pub fn execute(
    train_in: &Tensor,
    train_out: &Tensor,
    valid_in: &Tensor,
    valid_out: &Tensor,
    map: &VarMap,
    device: &Device,
    config: &Config,
) -> Result<()> {
    let variables = VarBuilder::from_varmap(map, candle_core::DType::F32, device);
    let network = Network::new(variables, config.vocab, config.dim, config.heads, config.limit, config.drop)?;

    let mut parameters = ParamsAdamW::default();
    parameters.lr = 0.001;
    let mut optimizer = AdamW::new(map.all_vars(), parameters)?;

    let (train_rows, train_cols) = train_in.dims2()?;
    let train_inputs = train_in.narrow(1, 0, train_cols - 1)?.contiguous()?;
    let train_targets = train_out.narrow(1, 1, train_cols - 1)?.contiguous()?;

    let (_valid_rows, valid_cols) = valid_in.dims2()?;
    let valid_inputs = valid_in.narrow(1, 0, valid_cols - 1)?.contiguous()?;
    let valid_targets = valid_out.narrow(1, 1, valid_cols - 1)?.contiguous()?.flatten_all()?;

    let batch = 32;
    let mut gen = rand::rng();
    let mut indices: Vec<u32> = (0..train_rows as u32).collect();

    for epoch in 0..150 {
        indices.shuffle(&mut gen);
        let mut total = 0.0;
        let mut steps = 0;

        for start in (0..train_rows).step_by(batch) {
            let end = usize::min(start + batch, train_rows);
            let current = end - start;
            let chunk = &indices[start..end];

            let tensor_indices = Tensor::from_vec(chunk.to_vec(), current, device)?;
            let batch_inputs = train_inputs.index_select(&tensor_indices, 0)?;
            let batch_targets = train_targets.index_select(&tensor_indices, 0)?.flatten_all()?;

            let predictions = network.forward(&batch_inputs, true)?;
            let reshaped = predictions.reshape(((), config.vocab))?;
            let error = loss::cross_entropy(&reshaped, &batch_targets)?;

            optimizer.backward_step(&error)?;
            total += error.to_scalar::<f32>()?;
            steps += 1;
        }

        let average = total / steps as f32;

        let valid_predictions = network.forward(&valid_inputs, false)?;
        let valid_reshaped = valid_predictions.reshape(((), config.vocab))?;
        let valid_error = loss::cross_entropy(&valid_reshaped, &valid_targets)?;
        let valid_value = valid_error.to_scalar::<f32>()?;

        println!("epoch {} train {} valid {}", epoch, average, valid_value);

        if average < 0.10 {
            break;
        }
    }

    map.save("weights.safetensors")?;

    Ok(())
}
