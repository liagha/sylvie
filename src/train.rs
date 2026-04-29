// src/train.rs
use crate::config::Config;
use crate::model::Network;
use candle_core::{Device, Result, Tensor};
use candle_nn::{loss, AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};

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
    let network = Network::new(variables, config.vocab, config.dim, config.heads, config.limit)?;

    let mut parameters = ParamsAdamW::default();
    parameters.lr = 0.001;
    let mut optimizer = AdamW::new(map.all_vars(), parameters)?;

    let (_t_batch, t_seq) = train_in.dims2()?;
    let t_input = train_in.narrow(1, 0, t_seq - 1)?;
    let t_target = train_out.narrow(1, 1, t_seq - 1)?.flatten_all()?;

    let (_v_batch, v_seq) = valid_in.dims2()?;
    let v_input = valid_in.narrow(1, 0, v_seq - 1)?;
    let v_target = valid_out.narrow(1, 1, v_seq - 1)?.flatten_all()?;

    for step in 0..5000 {
        let predictions = network.forward(&t_input)?;
        let reshaped = predictions.reshape(((), config.vocab))?;
        let error = loss::cross_entropy(&reshaped, &t_target)?;

        optimizer.backward_step(&error)?;
        let value = error.to_scalar::<f32>()?;

        if step % 100 == 0 {
            let valid_predictions = network.forward(&v_input)?;
            let valid_reshaped = valid_predictions.reshape(((), config.vocab))?;
            let valid_error = loss::cross_entropy(&valid_reshaped, &v_target)?;
            let valid_value = valid_error.to_scalar::<f32>()?;

            println!("step {} train {} valid {}", step, value, valid_value);
        }

        if value < 0.10 {
            println!("step {} train {}", step, value);
            break;
        }
    }

    map.save("weights.safetensors")?;

    Ok(())
}
