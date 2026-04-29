use std::fs;
use std::io::Write;
use tokenizers::{
    decoders::DecoderWrapper,
    models::bpe::{BpeTrainer, BPE},
    normalizers::{Lowercase, NormalizerWrapper, Sequence, Strip},
    pre_tokenizers::{whitespace::Whitespace, PreTokenizerWrapper},
    processors::PostProcessorWrapper,
    AddedToken, Tokenizer as Engine, TokenizerImpl,
};

pub struct Tokenizer {
    engine: Engine,
}

impl Tokenizer {
    pub fn train(iterator: impl IntoIterator<Item = String>) -> Self {
        let mut trainer = BpeTrainer::builder()
            .vocab_size(1000)
            .show_progress(false)
            .special_tokens(vec![
                AddedToken::from(String::from("<pad>"), true),
                AddedToken::from(String::from("<bos>"), true),
                AddedToken::from(String::from("<eos>"), true),
                AddedToken::from(String::from("<sep>"), true),
            ])
            .build();

        let mut engine: TokenizerImpl<
            BPE,
            NormalizerWrapper,
            PreTokenizerWrapper,
            PostProcessorWrapper,
            DecoderWrapper,
        > = TokenizerImpl::new(BPE::default());

        let _ = engine.with_normalizer(
            Sequence::new(vec![Strip::new(true, true).into(), Lowercase.into()]).into(),
        );
        let _ = engine.with_pre_tokenizer(Whitespace.into());

        let temp = "temp.txt";
        let mut file = fs::File::create(temp).unwrap();
        for text in iterator {
            writeln!(file, "{}", text).unwrap();
        }

        engine
            .train_from_files(&mut trainer, vec![temp.to_string()])
            .unwrap();

        engine.save("temp.json", true).unwrap();
        fs::remove_file(temp).unwrap();

        let wrapped = Engine::from_file("temp.json").unwrap();
        fs::remove_file("temp.json").unwrap();

        Self { engine: wrapped }
    }

    pub fn encode(&self, text: &str) -> Vec<u32> {
        self.engine.encode(text, false).unwrap().get_ids().to_vec()
    }

    pub fn decode(&self, tokens: &[u32]) -> String {
        let mut command = Vec::new();
        let mut active = false;

        for token in tokens {
            if *token == 2 {
                break;
            }
            if active {
                command.push(*token);
            }
            if *token == 3 {
                active = true;
            }
        }

        self.engine.decode(&command, true).unwrap()
    }

    pub fn size(&self) -> usize {
        self.engine.get_vocab_size(true)
    }

    pub fn save(&self, path: &str) {
        self.engine.save(path, true).unwrap();
    }

    pub fn load(path: &str) -> Self {
        Self {
            engine: Engine::from_file(path).unwrap(),
        }
    }
}