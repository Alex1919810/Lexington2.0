use super::resources::{self, Illustration};
use std::path::Path;

/// Resource selection and format fallback, with no GPU or UI state.
pub struct CharacterState {
    pub illustrations: Vec<Illustration>,
    pub selected: Option<usize>,
    pub character: String,
    pub format: String,
    pub texts: Vec<String>,
}
impl Default for CharacterState {
    fn default() -> Self {
        Self {
            illustrations: vec![],
            selected: None,
            character: String::new(),
            format: "gif".into(),
            texts: vec![],
        }
    }
}
impl CharacterState {
    pub fn characters(&self) -> Vec<String> {
        let mut names: Vec<_> = self
            .illustrations
            .iter()
            .map(|i| i.character.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }
    pub fn filtered(&self) -> Vec<usize> {
        self.illustrations
            .iter()
            .enumerate()
            .filter(|(_, i)| i.character == self.character && i.format == self.format)
            .map(|(n, _)| n)
            .collect()
    }
    pub fn selected_path(&self) -> Option<&Path> {
        self.selected.map(|i| self.illustrations[i].path.as_path())
    }
    pub fn select_first(&mut self) {
        if self.filtered().is_empty() {
            let other = if self.format == "gif" { "png" } else { "gif" };
            if self
                .illustrations
                .iter()
                .any(|ill| ill.character == self.character && ill.format == other)
            {
                self.format = other.into();
            }
        }
        self.selected = self.filtered().first().copied();
    }
    pub fn refresh(&mut self, root: &Path) {
        let old = self.selected.map(|i| self.illustrations[i].path.clone());
        self.illustrations = resources::illustrations(root);
        let names = self.characters();
        if !names.contains(&self.character) {
            self.character = names.first().cloned().unwrap_or_default();
        }
        if let Some(i) =
            old.and_then(|path| self.illustrations.iter().position(|ill| ill.path == path))
        {
            self.selected = Some(i);
        } else {
            self.select_first();
        }
        self.texts = resources::texts(root, &self.character);
    }
    pub fn next_illustration(&mut self) {
        let indices = self.filtered();
        let current = indices
            .iter()
            .position(|i| Some(*i) == self.selected)
            .unwrap_or(0);
        self.selected = indices.get((current + 1) % indices.len().max(1)).copied();
    }
    pub fn toggle_format(&mut self) {
        self.format = if self.format == "gif" { "png" } else { "gif" }.into();
        self.select_first();
    }
    pub fn next_character(&mut self, root: &Path) {
        let names = self.characters();
        let index = names.iter().position(|s| s == &self.character).unwrap_or(0);
        if !names.is_empty() {
            self.character = names[(index + 1) % names.len()].clone();
            self.texts = resources::texts(root, &self.character);
            self.select_first();
        }
    }
}
