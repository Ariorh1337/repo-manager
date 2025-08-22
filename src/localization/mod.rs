use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Localizer {
    current_language: String,
    translations: HashMap<String, HashMap<String, String>>,
}

impl Localizer {
    pub fn new(language: &str) -> Self {
        let translations = Self::load_translations();
        Self {
            current_language: language.to_string(),
            translations,
        }
    }

    fn load_translations() -> HashMap<String, HashMap<String, String>> {
        let mut all_translations = HashMap::new();

        let en_content = include_str!("../../assets/locales/en.json");
        if let Ok(en_translations) = serde_json::from_str::<HashMap<String, String>>(en_content) {
            all_translations.insert("en".to_string(), en_translations);
        }

        let ru_content = include_str!("../../assets/locales/ru.json");
        if let Ok(ru_translations) = serde_json::from_str::<HashMap<String, String>>(ru_content) {
            all_translations.insert("ru".to_string(), ru_translations);
        }

        if all_translations.is_empty() {
            let mut fallback = HashMap::new();
            fallback.insert("workspaces".to_string(), "Workspaces".to_string());
            fallback.insert("language".to_string(), "Language".to_string());
            fallback.insert("english".to_string(), "English".to_string());
            fallback.insert("russian".to_string(), "Russian".to_string());
            all_translations.insert("en".to_string(), fallback);
        }

        all_translations
    }

    pub fn set_language(&mut self, language: &str) {
        if self.translations.contains_key(language) {
            self.current_language = language.to_string();
        }
    }

    pub fn get_language(&self) -> &str {
        &self.current_language
    }

    pub fn t(&self, key: &str) -> String {
        if let Some(lang_map) = self.translations.get(&self.current_language) {
            if let Some(translation) = lang_map.get(key) {
                return translation.clone();
            }
        }

        if let Some(en_map) = self.translations.get("en") {
            if let Some(translation) = en_map.get(key) {
                return translation.clone();
            }
        }

        key.to_string()
    }

    pub fn tf(&self, key: &str, args: &[&str]) -> String {
        let template = self.t(key);
        let mut result = template.clone();

        for (i, arg) in args.iter().enumerate() {
            let placeholder = format!("{{{}}}", i);
            result = result.replace(&placeholder, arg);
        }

        for arg in args.iter() {
            if let Some(pos) = result.find("{}") {
                result.replace_range(pos..pos + 2, arg);
            } else {
                break;
            }
        }

        result
    }

    pub fn get_available_languages(&self) -> Vec<(&str, String)> {
        vec![("en", self.t("english")), ("ru", self.t("russian"))]
    }
}
