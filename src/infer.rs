use crate::config::Config;
use crate::model::Network;
use candle_core::{Device, Result, Tensor};
use candle_nn::{VarBuilder, VarMap};

pub fn execute(input: &[u32], device: &Device, config: &Config) -> Result<Vec<u32>> {
    let mut map = VarMap::new();

    let variables = VarBuilder::from_varmap(&map, candle_core::DType::F32, device);
    let network = Network::new(variables, config.vocab, config.dim, config.heads, config.limit)?;

    map.load("weights.safetensors")?;

    let mut sequence = input.to_vec();
    let limit = 20;

    for _ in 0..limit {
        let length = sequence.len();
        let tensor = Tensor::from_vec(sequence.clone(), (1, length), device)?;
        let logits = network.forward(&tensor)?;

        let array = logits.to_vec3::<f32>()?;
        let last = &array[0][length - 1];

        let mut index = 0;
        let mut highest = f32::NEG_INFINITY;

        for (idx, &val) in last.iter().enumerate() {
            if idx == 0 || idx == 1 || idx == 3 {
                continue;
            }
            if val > highest {
                highest = val;
                index = idx;
            }
        }

        sequence.push(index as u32);

        if index == 2 {
            break;
        }
    }

    Ok(sequence)
}
