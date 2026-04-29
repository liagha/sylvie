use crate::config::Config;
use crate::model::Network;
use candle_core::{Device, Result, Tensor};
use candle_nn::{loss, AdamW, Optimizer, ParamsAdamW, VarBuilder, VarMap};

pub fn execute(inputs: &Tensor, targets: &Tensor, map: &VarMap, device: &Device, config: &Config) -> Result<()> {
    let variables = VarBuilder::from_varmap(map, candle_core::DType::F32, device);
    let network = Network::new(variables, config.vocab, config.dim, config.heads, config.limit)?;

    let mut parameters = ParamsAdamW::default();
    parameters.lr = 0.001;
    let mut optimizer = AdamW::new(map.all_vars(), parameters)?;

    let (_batch, seq) = inputs.dims2()?;
    let input_slice = inputs.narrow(1, 0, seq - 1)?;
    let target_slice = targets.narrow(1, 1, seq - 1)?.flatten_all()?;

    for step in 0..5000 {
        let predictions = network.forward(&input_slice)?;
        let reshaped = predictions.reshape(((), config.vocab))?;
        let error = loss::cross_entropy(&reshaped, &target_slice)?;

        optimizer.backward_step(&error)?;

        let value = error.to_scalar::<f32>()?;

        if step % 100 == 0 {
            println!("step {} error {}", step, value);
        }

        if value < 0.31 {
            println!("step {} error {}", step, value);
            break;
        }
    }

    map.save("weights.safetensors")?;

    Ok(())
}
