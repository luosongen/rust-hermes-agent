//! 技能浏览模块

use crate::hub::error::HubError;
use crate::hub::index::SkillIndex;
use crate::hub::types::Category;

/// 技能浏览器
///
/// 提供分类和技能列表的浏览功能
pub struct Browse {
    index: SkillIndex,
}

impl Browse {
    /// 创建浏览器实例
    pub fn new(index: SkillIndex) -> Self {
        Self { index }
    }

    /// 列出所有分类
    pub fn list_categories(&self) -> Result<Vec<Category>, HubError> {
        self.index.get_categories()
    }

    /// 列出指定分类下的技能名称
    pub fn list_skills_in_category(&self, category: &str) -> Result<Vec<String>, HubError> {
        let skills = self.index.list_skills_by_category(category)?;
        Ok(skills.into_iter().map(|s| s.name).collect())
    }

    /// 打印分类列表
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

    /// 打印指定分类下的技能列表
    pub fn print_skill_list(&self, category: &str) -> Result<(), HubError> {
        let skills = self.index.list_skills_by_category(category)?;
        println!("\nSkills in {}:\n", category);
        for skill in skills {
            println!("  - {}: {}", skill.name, skill.description);
        }
        Ok(())
    }
}