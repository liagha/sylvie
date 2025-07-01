use linfa::prelude::*;
use linfa_bayes::GaussianNb;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use ndarray::{Array1, Array2};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskAction {
    OpenBrowser,
    OpenApp(String),
    SendEmail,
    CreateFile,
    PlayMusic,
    SetReminder,
    CheckWeather,
    SearchWeb,
    TakeScreenshot,
    OpenCalculator,
    Unknown,
}

impl TaskAction {
    pub fn from_label(label: &str) -> Self {
        match label {
            "open_browser" => TaskAction::OpenBrowser,
            "open_app" => TaskAction::OpenApp("default".to_string()),
            "send_email" => TaskAction::SendEmail,
            "create_file" => TaskAction::CreateFile,
            "play_music" => TaskAction::PlayMusic,
            "set_reminder" => TaskAction::SetReminder,
            "check_weather" => TaskAction::CheckWeather,
            "search_web" => TaskAction::SearchWeb,
            "take_screenshot" => TaskAction::TakeScreenshot,
            "open_calculator" => TaskAction::OpenCalculator,
            _ => TaskAction::Unknown,
        }
    }

    pub fn to_label(&self) -> &'static str {
        match self {
            TaskAction::OpenBrowser => "open_browser",
            TaskAction::OpenApp(_) => "open_app",
            TaskAction::SendEmail => "send_email",
            TaskAction::CreateFile => "create_file",
            TaskAction::PlayMusic => "play_music",
            TaskAction::SetReminder => "set_reminder",
            TaskAction::CheckWeather => "check_weather",
            TaskAction::SearchWeb => "search_web",
            TaskAction::TakeScreenshot => "take_screenshot",
            TaskAction::OpenCalculator => "open_calculator",
            TaskAction::Unknown => "unknown",
        }
    }
}

pub struct TextFeatureExtractor {
    vocabulary: HashMap<String, usize>,
    feature_size: usize,
}

impl TextFeatureExtractor {
    pub fn new() -> Self {
        Self {
            vocabulary: HashMap::new(),
            feature_size: 0,
        }
    }

    pub fn fit(&mut self, texts: &[String]) {
        let mut word_counts = HashMap::new();

        // Build vocabulary from all texts
        for text in texts {
            let words = self.tokenize(text);
            for word in words {
                *word_counts.entry(word).or_insert(0) += 1;
            }
        }

        // Keep words that appear at least twice
        let mut vocab_vec: Vec<_> = word_counts
            .into_iter()
            .filter(|(_, count)| *count >= 2)
            .map(|(word, _)| word)
            .collect();

        vocab_vec.sort();

        for (idx, word) in vocab_vec.into_iter().enumerate() {
            self.vocabulary.insert(word, idx);
        }

        self.feature_size = self.vocabulary.len();
    }

    pub fn transform(&self, text: &str) -> Vec<f64> {
        let mut features = vec![0.0; self.feature_size];
        let words = self.tokenize(text);

        // Simple bag-of-words with term frequency
        for word in words {
            if let Some(&idx) = self.vocabulary.get(&word) {
                features[idx] += 1.0;
            }
        }

        // Normalize by length
        let total: f64 = features.iter().sum();
        if total > 0.0 {
            for feature in &mut features {
                *feature /= total;
            }
        }

        features
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        text.to_lowercase()
            .split_whitespace()
            .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|s| !s.is_empty() && s.len() > 2)
            .map(|s| s.to_string())
            .collect()
    }
}

pub struct AIAssistant {
    model: Option<GaussianNb<f64, usize>>,
    feature_extractor: TextFeatureExtractor,
    label_mapping: HashMap<usize, String>,
}

impl AIAssistant {
    pub fn new() -> Self {
        Self {
            model: None,
            feature_extractor: TextFeatureExtractor::new(),
            label_mapping: HashMap::new(),
        }
    }

    pub fn get_training_data() -> (Vec<String>, Vec<String>) {
        let training_data = vec![
            ("open chrome", "open_browser"),
            ("launch browser", "open_browser"),
            ("start web browser", "open_browser"),
            ("open firefox", "open_browser"),
            ("browse the internet", "open_browser"),
            ("go online", "open_browser"),

            ("open notepad", "open_app"),
            ("launch calculator", "open_calculator"),
            ("start photoshop", "open_app"),
            ("run vlc", "open_app"),
            ("open spotify", "open_app"),
            ("launch code editor", "open_app"),
            ("start game", "open_app"),

            ("send email to john", "send_email"),
            ("compose email", "send_email"),
            ("write message", "send_email"),
            ("email my boss", "send_email"),
            ("send mail", "send_email"),

            ("create new file", "create_file"),
            ("make document", "create_file"),
            ("new text file", "create_file"),
            ("create folder", "create_file"),
            ("make new document", "create_file"),

            ("play music", "play_music"),
            ("start playlist", "play_music"),
            ("play songs", "play_music"),
            ("turn on music", "play_music"),
            ("play my favorite song", "play_music"),

            ("remind me to call mom", "set_reminder"),
            ("set alarm", "set_reminder"),
            ("create reminder", "set_reminder"),
            ("schedule meeting", "set_reminder"),
            ("remind me tomorrow", "set_reminder"),

            ("what's the weather", "check_weather"),
            ("weather forecast", "check_weather"),
            ("is it raining", "check_weather"),
            ("temperature today", "check_weather"),
            ("weather report", "check_weather"),

            ("search for restaurants", "search_web"),
            ("google something", "search_web"),
            ("look up information", "search_web"),
            ("find tutorial", "search_web"),
            ("search online", "search_web"),

            ("take screenshot", "take_screenshot"),
            ("capture screen", "take_screenshot"),
            ("screen grab", "take_screenshot"),
            ("save screenshot", "take_screenshot"),

            ("open calculator", "open_calculator"),
            ("launch calc", "open_calculator"),
            ("start calculator", "open_calculator"),
            ("calculate something", "open_calculator"),
        ];

        let (texts, labels): (Vec<_>, Vec<_>) = training_data.into_iter().unzip();
        (texts.into_iter().map(|s| s.to_string()).collect(),
         labels.into_iter().map(|s| s.to_string()).collect())
    }

    pub fn train(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let (texts, labels) = Self::get_training_data();

        // Fit feature extractor
        self.feature_extractor.fit(&texts);

        // Create feature matrix
        let features: Vec<Vec<f64>> = texts.iter()
            .map(|text| self.feature_extractor.transform(text))
            .collect();

        // Create label mapping
        let unique_labels: std::collections::HashSet<_> = labels.iter().cloned().collect();
        for (idx, label) in unique_labels.iter().enumerate() {
            self.label_mapping.insert(idx, label.clone());
        }

        // Convert labels to indices
        let label_to_idx: HashMap<String, usize> = self.label_mapping.iter()
            .map(|(idx, label)| (label.clone(), *idx))
            .collect();

        let label_indices: Vec<usize> = labels.iter()
            .map(|label| *label_to_idx.get(label).unwrap())
            .collect();

        // Create dataset
        let feature_matrix = Array2::from_shape_vec(
            (features.len(), features[0].len()),
            features.into_iter().flatten().collect(),
        )?;

        let target_array = Array1::from_vec(label_indices);
        let dataset = Dataset::new(feature_matrix, target_array);

        // Train model
        self.model = Some(GaussianNb::params().fit(&dataset)?);
        println!("Model trained successfully with {} features", self.feature_extractor.feature_size);

        Ok(())
    }

    pub fn predict(&self, input: &str) -> Result<TaskAction, Box<dyn std::error::Error>> {
        let model = self.model.as_ref().ok_or("Model not trained")?;

        // Extract features
        let features = self.feature_extractor.transform(input);
        let feature_array = Array2::from_shape_vec((1, features.len()), features)?;

        // Predict
        let prediction = model.predict(&feature_array);
        let predicted_label_idx = prediction[0];

        // Convert back to action
        let label = self.label_mapping.get(&predicted_label_idx)
            .ok_or("Invalid prediction")?;

        let mut action = TaskAction::from_label(label);

        // Handle app-specific cases
        if matches!(action, TaskAction::OpenApp(_)) {
            if input.to_lowercase().contains("calculator") || input.to_lowercase().contains("calc") {
                action = TaskAction::OpenCalculator;
            } else {
                // Extract app name from input
                let app_name = self.extract_app_name(input);
                action = TaskAction::OpenApp(app_name);
            }
        }

        Ok(action)
    }

    fn extract_app_name(&self, input: &str) -> String {
        let words: Vec<&str> = input.split_whitespace().collect();

        // Look for common app indicators
        for (i, word) in words.iter().enumerate() {
            if ["open", "launch", "start", "run"].contains(&word.to_lowercase().as_str()) {
                if i + 1 < words.len() {
                    return words[i + 1].to_string();
                }
            }
        }

        // Fallback to last word
        words.last().unwrap_or(&"unknown").to_string()
    }

    pub fn save_model(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let serialized = bincode::serialize(&(&self.feature_extractor.vocabulary, &self.label_mapping))?;
        fs::write(path, serialized)?;
        println!("Model saved to {}", path);
        Ok(())
    }

    pub fn load_model(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let data = fs::read(path)?;
        let (vocabulary, label_mapping): (HashMap<String, usize>, HashMap<usize, String>) =
            bincode::deserialize(&data)?;

        self.feature_extractor.vocabulary = vocabulary;
        self.feature_extractor.feature_size = self.feature_extractor.vocabulary.len();
        self.label_mapping = label_mapping;

        // Retrain model (since we can't easily serialize the Linfa model)
        self.train()?;
        println!("Model loaded from {}", path);
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 AI Assistant Training and Inference");
    println!("=====================================");

    let mut assistant = AIAssistant::new();

    // Check if model exists
    let model_path = "assistant_model.bin";
    if fs::metadata(model_path).is_ok() {
        println!("Loading existing model...");
        assistant.load_model(model_path)?;
    } else {
        println!("Training new model...");
        assistant.train()?;
        assistant.save_model(model_path)?;
    }

    println!("\n✅ Ready! Enter commands (type 'quit' to exit):");
    println!("Examples: 'open browser', 'play music', 'send email', 'take screenshot'");

    loop {
        print!("\n> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        if input.to_lowercase() == "quit" {
            break;
        }

        match assistant.predict(input) {
            Ok(action) => {
                println!("🎯 Predicted action: {:?}", action);
            }
            Err(e) => {
                println!("❌ Error: {}", e);
            }
        }
    }

    println!("👋 Goodbye!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_action_conversion() {
        assert_eq!(TaskAction::from_label("open_browser"), TaskAction::OpenBrowser);
        assert_eq!(TaskAction::OpenBrowser.to_label(), "open_browser");
    }

    #[test]
    fn test_feature_extraction() {
        let mut extractor = TextFeatureExtractor::new();
        let texts = vec!["open browser".to_string(), "launch app".to_string()];
        extractor.fit(&texts);

        let features = extractor.transform("open browser");
        assert!(!features.is_empty());
    }

    #[test]
    fn test_training_and_prediction() {
        let mut assistant = AIAssistant::new();
        assistant.train().unwrap();

        let result = assistant.predict("open chrome").unwrap();
        assert_eq!(result, TaskAction::OpenBrowser);
    }
}