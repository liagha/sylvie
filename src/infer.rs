use crate::config::Config;
use crate::model::Network;
use candle_core::{Device, Result, Tensor};
use candle_nn::{VarBuilder, VarMap};
use candle_transformers::generation::LogitsProcessor;

pub fn execute(input: &[u32], device: &Device, config: &Config) -> Result<Vec<u32>> {
    let mut map = VarMap::new();

    let variables = VarBuilder::from_varmap(&map, candle_core::DType::F32, device);
    let network = Network::new(variables, config.vocab, config.dim, config.heads, config.limit, config.drop, config.layers)?;

    map.load("weights.safetensors")?;

    let mut sequence = input.to_vec();
    let limit = 20;
    let mut processor = LogitsProcessor::new(42, Some(0.5), Some(50.0));

    for _ in 0..limit {
        let length = sequence.len();
        let tensor = Tensor::from_vec(sequence.clone(), (1, length), device)?;
        let logits = network.forward(&tensor, false)?;

        let last = logits.narrow(1, length - 1, 1)?.squeeze(1)?.squeeze(0)?;
        let mut array = last.to_vec1::<f32>()?;

        array[0] = f32::NEG_INFINITY;
        array[1] = f32::NEG_INFINITY;

        let masked = Tensor::from_vec(array, config.vocab, device)?;
        let index = processor.sample(&masked)?;

        sequence.push(index);

        if index == 2 {
            break;
        }
    }

    Ok(sequence)
}