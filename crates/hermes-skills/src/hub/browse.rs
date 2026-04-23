use crate::hub::error::HubError;
use crate::hub::index::SkillIndex;
use crate::hub::types::Category;

pub struct Browse {
    index: SkillIndex,
}

impl Browse {
    pub fn new(index: SkillIndex) -> Self {
        Self { index }
    }

    pub fn list_categories(&self) -> Result<Vec<Category>, HubError> {
        self.index.get_categories()
    }

    pub fn list_skills_in_category(&self, category: &str) -> Result<Vec<String>, HubError> {
        let skills = self.index.list_skills_by_category(category)?;
        Ok(skills.into_iter().map(|s| s.name).collect())
    }

    pub fn print_category_list(&self) -> Result<(), HubError> {
        let categories = self.list_categories()?;
        println!("Available categories:\n");
        for (i, cat) in categories.iter().enumerate() {
            println!("  {}. {} ({})", i + 1, cat.name, cat.skill_count);
            if !cat.description.is_empty() {
                println!("     {}", cat.description);
            }
        }
        Ok(())
    }

    pub fn print_skill_list(&self, category: &str) -> Result<(), HubError> {
        let skills = self.index.list_skills_by_category(category)?;
        println!("\nSkills in {}:\n", category);
        for skill in skills {
            println!("  - {}: {}", skill.name, skill.description);
        }
        Ok(())
    }
}