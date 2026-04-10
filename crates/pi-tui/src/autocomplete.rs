//! 自动完成系统
//! 提供自动完成建议功能和 Slash 命令支持

use crate::fuzzy::fuzzy_filter;

/// 自动完成建议项
#[derive(Debug, Clone)]
pub struct AutocompleteItem {
    /// 显示标签
    pub label: String,
    /// 详细描述
    pub detail: Option<String>,
    /// 插入文本（如果与 label 不同）
    pub insert_text: Option<String>,
    /// 项目类型
    pub kind: Option<String>,
}

impl AutocompleteItem {
    /// 创建新的建议项
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            insert_text: None,
            kind: None,
        }
    }

    /// 设置详细描述
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// 设置插入文本
    pub fn with_insert_text(mut self, text: impl Into<String>) -> Self {
        self.insert_text = Some(text.into());
        self
    }

    /// 设置类型
    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    /// 获取要插入的文本
    pub fn get_insert_text(&self) -> &str {
        self.insert_text.as_deref().unwrap_or(&self.label)
    }
}

/// 自动完成建议
#[derive(Debug, Clone)]
pub struct AutocompleteSuggestions {
    /// 建议项列表
    pub items: Vec<AutocompleteItem>,
    /// 已输入的前缀
    pub prefix: String,
}

impl AutocompleteSuggestions {
    /// 创建新的建议集合
    pub fn new(items: Vec<AutocompleteItem>, prefix: impl Into<String>) -> Self {
        Self {
            items,
            prefix: prefix.into(),
        }
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 获取建议数量
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

/// Slash 命令
#[derive(Debug, Clone)]
pub struct SlashCommand {
    /// 命令名称
    pub name: String,
    /// 命令描述
    pub description: String,
    /// 命令别名
    pub aliases: Vec<String>,
}

impl SlashCommand {
    /// 创建新的 Slash 命令
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            aliases: Vec::new(),
        }
    }

    /// 添加别名
    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    /// 添加多个别名
    pub fn with_aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases.extend(aliases);
        self
    }

    /// 检查名称是否匹配（包括别名）
    pub fn matches(&self, name: &str) -> bool {
        if self.name.eq_ignore_ascii_case(name) {
            return true;
        }
        self.aliases.iter().any(|a| a.eq_ignore_ascii_case(name))
    }
}

/// 自动完成提供者 trait
pub trait AutocompleteProvider: Send + Sync {
    /// 提供自动完成建议
    /// 
    /// # Arguments
    /// * `input` - 当前输入文本
    /// * `cursor_pos` - 光标位置（字节索引）
    /// 
    /// # Returns
    /// 如果有建议返回 Some，否则返回 None
    fn provide(&self, input: &str, cursor_pos: usize) -> Option<AutocompleteSuggestions>;
}

/// 组合多个提供者
pub struct CombinedAutocompleteProvider {
    providers: Vec<Box<dyn AutocompleteProvider>>,
}

impl CombinedAutocompleteProvider {
    /// 创建新的组合提供者
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// 添加提供者
    pub fn add(&mut self, provider: Box<dyn AutocompleteProvider>) {
        self.providers.push(provider);
    }

    /// 获取提供者数量
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl Default for CombinedAutocompleteProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AutocompleteProvider for CombinedAutocompleteProvider {
    fn provide(&self, input: &str, cursor_pos: usize) -> Option<AutocompleteSuggestions> {
        for provider in &self.providers {
            if let Some(suggestions) = provider.provide(input, cursor_pos) {
                return Some(suggestions);
            }
        }
        None
    }
}

/// 简单的关键字提供者
pub struct KeywordAutocompleteProvider {
    keywords: Vec<String>,
}

impl KeywordAutocompleteProvider {
    /// 创建新的关键字提供者
    pub fn new(keywords: Vec<String>) -> Self {
        Self { keywords }
    }

    /// 添加关键字
    pub fn add_keyword(&mut self, keyword: impl Into<String>) {
        self.keywords.push(keyword.into());
    }
}

impl AutocompleteProvider for KeywordAutocompleteProvider {
    fn provide(&self, input: &str, cursor_pos: usize) -> Option<AutocompleteSuggestions> {
        let before_cursor = &input[..cursor_pos.min(input.len())];
        
        // 获取当前词
        let word_start = before_cursor
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);
        let prefix = &before_cursor[word_start..];

        if prefix.is_empty() {
            return None;
        }

        // 使用模糊匹配过滤关键字
        let items: Vec<AutocompleteItem> = self
            .keywords
            .iter()
            .filter_map(|kw| {
                if kw.starts_with(prefix) {
                    Some(AutocompleteItem::new(kw.clone()))
                } else {
                    None
                }
            })
            .collect();

        if items.is_empty() {
            None
        } else {
            Some(AutocompleteSuggestions::new(items, prefix))
        }
    }
}

/// Slash 命令提供者
pub struct SlashCommandProvider {
    commands: Vec<SlashCommand>,
}

impl SlashCommandProvider {
    /// 创建新的 Slash 命令提供者
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// 添加命令
    pub fn add_command(&mut self, command: SlashCommand) {
        self.commands.push(command);
    }

    /// 从列表创建
    pub fn from_commands(commands: Vec<SlashCommand>) -> Self {
        Self { commands }
    }
}

impl Default for SlashCommandProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AutocompleteProvider for SlashCommandProvider {
    fn provide(&self, input: &str, cursor_pos: usize) -> Option<AutocompleteSuggestions> {
        let before_cursor = &input[..cursor_pos.min(input.len())];

        // 检查是否以 / 开头
        if !before_cursor.starts_with('/') {
            return None;
        }

        // 提取命令前缀
        let prefix = &before_cursor[1..]; // 去掉 /

        // 如果在空格后，不提供建议
        if prefix.contains(' ') {
            return None;
        }

        // 使用模糊匹配过滤命令
        let matches = fuzzy_filter(prefix, &self.commands, |cmd| &cmd.name);

        if matches.is_empty() {
            return None;
        }

        let items: Vec<AutocompleteItem> = matches
            .into_iter()
            .map(|(_, m)| {
                let cmd = &self.commands[m.indices[0]];
                AutocompleteItem::new(cmd.name.clone())
                    .with_detail(&cmd.description)
                    .with_kind("command")
            })
            .collect();

        Some(AutocompleteSuggestions::new(items, before_cursor))
    }
}

/// 文件路径提供者（简化版）
pub struct FilePathAutocompleteProvider;

impl FilePathAutocompleteProvider {
    /// 创建新的文件路径提供者
    pub fn new() -> Self {
        Self
    }
}

impl Default for FilePathAutocompleteProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AutocompleteProvider for FilePathAutocompleteProvider {
    fn provide(&self, input: &str, cursor_pos: usize) -> Option<AutocompleteSuggestions> {
        let before_cursor = &input[..cursor_pos.min(input.len())];

        // 检查是否包含路径分隔符或看起来像路径
        if !before_cursor.contains('/') && !before_cursor.contains('.') && !before_cursor.starts_with('~') {
            return None;
        }

        // 简化实现：实际应该读取目录内容
        // 这里返回 None，表示不提供建议
        None
    }
}

/// 应用自动完成建议
/// 
/// # Arguments
/// * `input` - 原始输入
/// * `cursor_pos` - 光标位置
/// * `item` - 选中的建议项
/// * `prefix` - 当前前缀
/// 
/// # Returns
/// 返回 (新文本, 新光标位置)
pub fn apply_completion(
    input: &str,
    cursor_pos: usize,
    item: &AutocompleteItem,
    prefix: &str,
) -> (String, usize) {
    let before_prefix = &input[..cursor_pos.saturating_sub(prefix.len())];
    let after_cursor = &input[cursor_pos.min(input.len())..];

    let insert_text = item.get_insert_text();
    let new_text = format!("{}{}{}", before_prefix, insert_text, after_cursor);
    let new_cursor = before_prefix.len() + insert_text.len();

    (new_text, new_cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autocomplete_item() {
        let item = AutocompleteItem::new("test")
            .with_detail("description")
            .with_kind("keyword");

        assert_eq!(item.label, "test");
        assert_eq!(item.detail, Some("description".to_string()));
        assert_eq!(item.get_insert_text(), "test");
    }

    #[test]
    fn test_autocomplete_item_with_insert_text() {
        let item = AutocompleteItem::new("display")
            .with_insert_text("actual_insert");

        assert_eq!(item.get_insert_text(), "actual_insert");
    }

    #[test]
    fn test_slash_command() {
        let cmd = SlashCommand::new("help", "Show help")
            .with_alias("h")
            .with_alias("?");

        assert!(cmd.matches("help"));
        assert!(cmd.matches("HELP"));
        assert!(cmd.matches("h"));
        assert!(!cmd.matches("unknown"));
    }

    #[test]
    fn test_keyword_provider() {
        let provider = KeywordAutocompleteProvider::new(vec![
            "function".to_string(),
            "const".to_string(),
            "let".to_string(),
        ]);

        // 匹配前缀
        let result = provider.provide("fun", 3);
        assert!(result.is_some());
        let suggestions = result.unwrap();
        assert!(!suggestions.is_empty());

        // 不匹配
        let result = provider.provide("xyz", 3);
        assert!(result.is_none());
    }

    #[test]
    fn test_slash_command_provider() {
        let mut provider = SlashCommandProvider::new();
        provider.add_command(SlashCommand::new("help", "Show help"));
        provider.add_command(SlashCommand::new("exit", "Exit application"));

        // 匹配 /h
        let result = provider.provide("/h", 2);
        assert!(result.is_some());

        // 不匹配普通文本
        let result = provider.provide("hello", 5);
        assert!(result.is_none());
    }

    #[test]
    fn test_combined_provider() {
        let mut combined = CombinedAutocompleteProvider::new();
        combined.add(Box::new(SlashCommandProvider::new()));
        combined.add(Box::new(KeywordAutocompleteProvider::new(vec![])));

        assert_eq!(combined.len(), 2);
    }

    #[test]
    fn test_apply_completion() {
        let item = AutocompleteItem::new("function");
        // 注意：输入不应该包含光标标记 |，cursor_pos 直接指定光标位置
        let (new_text, new_pos) = apply_completion("fun rest", 3, &item, "fun");

        assert_eq!(new_text, "function rest");
        assert_eq!(new_pos, 8);
    }

    #[test]
    fn test_apply_completion_with_insert_text() {
        let item = AutocompleteItem::new("fn")
            .with_insert_text("function");
        let (new_text, new_pos) = apply_completion("f", 1, &item, "f");

        assert_eq!(new_text, "function");
        assert_eq!(new_pos, 8);
    }
}
