use std::collections::HashMap;

#[derive(Clone)]
pub struct Vocab {
    string_to_token: HashMap<String, usize>,
}

impl Vocab {
    pub fn new(data: &[Vec<String>]) -> Self {
        let mut string_to_token = HashMap::new();
        string_to_token.insert("<PAD>".to_string(), 0);
        string_to_token.insert("<UNK>".to_string(), 1);

        for list_of_strs in data {
            for s in list_of_strs {
                for word in s.to_lowercase().split_whitespace() {
                    let len = string_to_token.len();
                    string_to_token.entry(word.to_string()).or_insert(len);
                }
            }
        }

        Self { string_to_token }
    }

    pub fn encode(&self, text: &str, max_len: usize) -> Vec<usize> {
        let mut tokens: Vec<usize> = text
            .to_lowercase()
            .split_whitespace()
            .take(max_len)
            .map(|w| *self.string_to_token.get(w).unwrap_or(&1))
            .collect();

        tokens.resize(max_len, 0);
        tokens
    }

    pub fn vocab_size(&self) -> usize {
        self.string_to_token.len()
    }
}
