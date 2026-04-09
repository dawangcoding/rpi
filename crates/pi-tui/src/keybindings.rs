//! 快捷键管理模块
//!
//! 提供快捷键定义、冲突检测和全局管理功能

use crate::keys::{matches_key, Key};
use std::collections::HashMap;
use std::sync::{Arc, RwLock, OnceLock};

// =============================================================================
// 类型定义
// =============================================================================

/// 快捷键定义
#[derive(Debug, Clone)]
pub struct KeybindingDefinition {
    /// 按键描述（如 "ctrl+c", "escape", "enter"）
    pub key: String,
    /// 操作名称（如 "cancel", "submit", "newline"）
    pub action: String,
    /// 描述
    pub description: String,
    /// 可选的上下文限定
    pub context: Option<String>,
}

impl KeybindingDefinition {
    /// 创建新的快捷键定义
    pub fn new(key: impl Into<String>, action: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            action: action.into(),
            description: description.into(),
            context: None,
        }
    }

    /// 添加上下文
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
}

/// 快捷键配置
#[derive(Debug, Clone, Default)]
pub struct KeybindingsConfig {
    pub bindings: Vec<KeybindingDefinition>,
}

impl KeybindingsConfig {
    /// 创建新的空配置
    pub fn new() -> Self {
        Self { bindings: Vec::new() }
    }

    /// 添加绑定
    pub fn add(&mut self, definition: KeybindingDefinition) {
        self.bindings.push(definition);
    }

    /// 从默认配置创建
    pub fn default_bindings() -> Self {
        default_keybindings()
    }
}

/// 快捷键冲突
#[derive(Debug, Clone)]
pub struct KeybindingConflict {
    /// 冲突的按键
    pub key: String,
    /// 冲突的操作列表
    pub actions: Vec<String>,
}

// =============================================================================
// 快捷键管理器
// =============================================================================

/// 快捷键管理器
pub struct KeybindingsManager {
    /// 所有绑定
    bindings: Vec<KeybindingDefinition>,
    /// 按键到操作的快速查找映射
    key_to_actions: HashMap<String, Vec<String>>,
    /// 操作到按键的映射
    action_to_keys: HashMap<String, Vec<String>>,
}

impl KeybindingsManager {
    /// 创建新的快捷键管理器
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            key_to_actions: HashMap::new(),
            action_to_keys: HashMap::new(),
        }
    }

    /// 从配置创建
    pub fn from_config(config: KeybindingsConfig) -> Self {
        let mut manager = Self::new();
        for binding in config.bindings {
            manager.add(binding);
        }
        manager
    }

    /// 重建查找映射
    fn rebuild_maps(&mut self) {
        self.key_to_actions.clear();
        self.action_to_keys.clear();

        for binding in &self.bindings {
            self.key_to_actions
                .entry(binding.key.clone())
                .or_default()
                .push(binding.action.clone());

            self.action_to_keys
                .entry(binding.action.clone())
                .or_default()
                .push(binding.key.clone());
        }
    }

    /// 查找匹配的操作
    /// 返回第一个匹配的操作名称
    pub fn find_action(&self, key: &Key, context: Option<&str>) -> Option<&str> {
        for binding in &self.bindings {
            // 如果指定了上下文，检查是否匹配
            if let Some(ctx) = &binding.context {
                if let Some(req_ctx) = context {
                    if ctx != req_ctx {
                        continue;
                    }
                }
            }

            if matches_key(key, &binding.key) {
                return Some(&binding.action);
            }
        }
        None
    }

    /// 查找所有匹配的操作
    pub fn find_all_actions(&self, key: &Key, context: Option<&str>) -> Vec<&str> {
        let mut actions = Vec::new();
        for binding in &self.bindings {
            // 如果指定了上下文，检查是否匹配
            if let Some(ctx) = &binding.context {
                if let Some(req_ctx) = context {
                    if ctx != req_ctx {
                        continue;
                    }
                }
            }

            if matches_key(key, &binding.key) {
                actions.push(binding.action.as_str());
            }
        }
        actions
    }

    /// 检测冲突
    pub fn find_conflicts(&self) -> Vec<KeybindingConflict> {
        let mut conflicts = Vec::new();

        for (key, actions) in &self.key_to_actions {
            if actions.len() > 1 {
                conflicts.push(KeybindingConflict {
                    key: key.clone(),
                    actions: actions.clone(),
                });
            }
        }

        conflicts
    }

    /// 添加绑定
    pub fn add(&mut self, definition: KeybindingDefinition) {
        // 检查是否已存在相同的按键+操作组合
        let exists = self.bindings.iter().any(|b| {
            b.key == definition.key && b.action == definition.action && b.context == definition.context
        });

        if !exists {
            self.bindings.push(definition);
            self.rebuild_maps();
        }
    }

    /// 移除绑定
    pub fn remove(&mut self, key: &str, action: &str) {
        self.bindings.retain(|b| !(b.key == key && b.action == action));
        self.rebuild_maps();
    }

    /// 根据操作移除所有绑定
    pub fn remove_by_action(&mut self, action: &str) {
        self.bindings.retain(|b| b.action != action);
        self.rebuild_maps();
    }

    /// 根据按键移除所有绑定
    pub fn remove_by_key(&mut self, key: &str) {
        self.bindings.retain(|b| b.key != key);
        self.rebuild_maps();
    }

    /// 获取所有绑定
    pub fn get_bindings(&self) -> &[KeybindingDefinition] {
        &self.bindings
    }

    /// 获取操作的按键
    pub fn get_keys_for_action(&self, action: &str) -> Option<&[String]> {
        self.action_to_keys.get(action).map(|v| v.as_slice())
    }

    /// 获取按键的操作
    pub fn get_actions_for_key(&self, key: &str) -> Option<&[String]> {
        self.key_to_actions.get(key).map(|v| v.as_slice())
    }

    /// 检查按键是否有绑定
    pub fn has_binding(&self, key: &str) -> bool {
        self.key_to_actions.contains_key(key)
    }

    /// 检查操作是否有绑定
    pub fn has_action(&self, action: &str) -> bool {
        self.action_to_keys.contains_key(action)
    }

    /// 清空所有绑定
    pub fn clear(&mut self) {
        self.bindings.clear();
        self.key_to_actions.clear();
        self.action_to_keys.clear();
    }

    /// 获取绑定数量
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl Default for KeybindingsManager {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// 默认快捷键
// =============================================================================

/// 默认快捷键映射
pub fn default_keybindings() -> KeybindingsConfig {
    KeybindingsConfig {
        bindings: vec![
            // 取消/退出
            KeybindingDefinition {
                key: "ctrl+c".into(),
                action: "cancel".into(),
                description: "Cancel current operation".into(),
                context: None,
            },
            KeybindingDefinition {
                key: "ctrl+d".into(),
                action: "exit".into(),
                description: "Exit".into(),
                context: None,
            },
            KeybindingDefinition {
                key: "ctrl+q".into(),
                action: "quit".into(),
                description: "Quit application".into(),
                context: None,
            },
            // 输入操作
            KeybindingDefinition {
                key: "enter".into(),
                action: "submit".into(),
                description: "Submit input".into(),
                context: Some("input".into()),
            },
            KeybindingDefinition {
                key: "shift+enter".into(),
                action: "newline".into(),
                description: "Insert newline".into(),
                context: Some("input".into()),
            },
            KeybindingDefinition {
                key: "escape".into(),
                action: "dismiss".into(),
                description: "Dismiss overlay".into(),
                context: Some("overlay".into()),
            },
            // 编辑器操作
            KeybindingDefinition {
                key: "ctrl+z".into(),
                action: "undo".into(),
                description: "Undo".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+shift+z".into(),
                action: "redo".into(),
                description: "Redo".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+y".into(),
                action: "redo".into(),
                description: "Redo (alternative)".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+x".into(),
                action: "cut".into(),
                description: "Cut selection".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+c".into(),
                action: "copy".into(),
                description: "Copy selection".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+v".into(),
                action: "paste".into(),
                description: "Paste".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+a".into(),
                action: "select_all".into(),
                description: "Select all".into(),
                context: Some("editor".into()),
            },
            // 光标移动
            KeybindingDefinition {
                key: "up".into(),
                action: "cursor_up".into(),
                description: "Move cursor up".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "down".into(),
                action: "cursor_down".into(),
                description: "Move cursor down".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "left".into(),
                action: "cursor_left".into(),
                description: "Move cursor left".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "right".into(),
                action: "cursor_right".into(),
                description: "Move cursor right".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+left".into(),
                action: "word_left".into(),
                description: "Move cursor word left".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+right".into(),
                action: "word_right".into(),
                description: "Move cursor word right".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "home".into(),
                action: "line_start".into(),
                description: "Move to line start".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "end".into(),
                action: "line_end".into(),
                description: "Move to line end".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+home".into(),
                action: "doc_start".into(),
                description: "Move to document start".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+end".into(),
                action: "doc_end".into(),
                description: "Move to document end".into(),
                context: Some("editor".into()),
            },
            // 删除操作
            KeybindingDefinition {
                key: "backspace".into(),
                action: "delete_backward".into(),
                description: "Delete character backward".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "delete".into(),
                action: "delete_forward".into(),
                description: "Delete character forward".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+w".into(),
                action: "delete_word_backward".into(),
                description: "Delete word backward".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "alt+backspace".into(),
                action: "delete_word_backward".into(),
                description: "Delete word backward".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+delete".into(),
                action: "delete_word_forward".into(),
                description: "Delete word forward".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "alt+delete".into(),
                action: "delete_word_forward".into(),
                description: "Delete word forward".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+u".into(),
                action: "delete_to_line_start".into(),
                description: "Delete to line start".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "ctrl+k".into(),
                action: "delete_to_line_end".into(),
                description: "Delete to line end".into(),
                context: Some("editor".into()),
            },
            // 页面滚动
            KeybindingDefinition {
                key: "pageup".into(),
                action: "page_up".into(),
                description: "Page up".into(),
                context: Some("editor".into()),
            },
            KeybindingDefinition {
                key: "pagedown".into(),
                action: "page_down".into(),
                description: "Page down".into(),
                context: Some("editor".into()),
            },
            // 自动完成
            KeybindingDefinition {
                key: "tab".into(),
                action: "accept_completion".into(),
                description: "Accept autocomplete suggestion".into(),
                context: Some("autocomplete".into()),
            },
            KeybindingDefinition {
                key: "ctrl+n".into(),
                action: "next_completion".into(),
                description: "Next autocomplete suggestion".into(),
                context: Some("autocomplete".into()),
            },
            KeybindingDefinition {
                key: "ctrl+p".into(),
                action: "prev_completion".into(),
                description: "Previous autocomplete suggestion".into(),
                context: Some("autocomplete".into()),
            },
            KeybindingDefinition {
                key: "escape".into(),
                action: "cancel_completion".into(),
                description: "Cancel autocomplete".into(),
                context: Some("autocomplete".into()),
            },
            // 搜索
            KeybindingDefinition {
                key: "ctrl+f".into(),
                action: "find".into(),
                description: "Find".into(),
                context: None,
            },
            KeybindingDefinition {
                key: "ctrl+g".into(),
                action: "find_next".into(),
                description: "Find next".into(),
                context: Some("search".into()),
            },
            KeybindingDefinition {
                key: "ctrl+shift+g".into(),
                action: "find_prev".into(),
                description: "Find previous".into(),
                context: Some("search".into()),
            },
            KeybindingDefinition {
                key: "escape".into(),
                action: "close_search".into(),
                description: "Close search".into(),
                context: Some("search".into()),
            },
            // 其他
            KeybindingDefinition {
                key: "ctrl+l".into(),
                action: "clear".into(),
                description: "Clear screen".into(),
                context: None,
            },
            KeybindingDefinition {
                key: "ctrl+s".into(),
                action: "save".into(),
                description: "Save".into(),
                context: None,
            },
            KeybindingDefinition {
                key: "ctrl+o".into(),
                action: "open".into(),
                description: "Open".into(),
                context: None,
            },
            KeybindingDefinition {
                key: "f1".into(),
                action: "help".into(),
                description: "Show help".into(),
                context: None,
            },
            KeybindingDefinition {
                key: "ctrl+equal".into(),
                action: "zoom_in".into(),
                description: "Zoom in".into(),
                context: None,
            },
            KeybindingDefinition {
                key: "ctrl+minus".into(),
                action: "zoom_out".into(),
                description: "Zoom out".into(),
                context: None,
            },
            KeybindingDefinition {
                key: "ctrl+0".into(),
                action: "zoom_reset".into(),
                description: "Reset zoom".into(),
                context: None,
            },
        ],
    }
}

// =============================================================================
// 全局快捷键管理
// =============================================================================

static GLOBAL_KEYBINDINGS: OnceLock<Arc<RwLock<KeybindingsManager>>> = OnceLock::new();

/// 获取全局快捷键管理器
pub fn get_keybindings() -> Arc<RwLock<KeybindingsManager>> {
    GLOBAL_KEYBINDINGS
        .get_or_init(|| Arc::new(RwLock::new(KeybindingsManager::from_config(default_keybindings()))))
        .clone()
}

/// 设置全局快捷键配置
pub fn set_keybindings(config: KeybindingsConfig) {
    if let Ok(mut manager) = get_keybindings().write() {
        *manager = KeybindingsManager::from_config(config);
    }
}

/// 重置为默认快捷键
pub fn reset_to_default_keybindings() {
    set_keybindings(default_keybindings());
}

// =============================================================================
// 便捷函数
// =============================================================================

/// 查找按键对应的操作
pub fn find_action(key: &Key, context: Option<&str>) -> Option<String> {
    if let Ok(manager) = get_keybindings().read() {
        manager.find_action(key, context).map(|s| s.to_string())
    } else {
        None
    }
}

/// 检查按键是否匹配某个操作
pub fn is_action(key: &Key, action: &str, context: Option<&str>) -> bool {
    if let Ok(manager) = get_keybindings().read() {
        manager.find_action(key, context) == Some(action)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keybinding_definition() {
        let def = KeybindingDefinition::new("ctrl+c", "cancel", "Cancel operation")
            .with_context("global");

        assert_eq!(def.key, "ctrl+c");
        assert_eq!(def.action, "cancel");
        assert_eq!(def.context, Some("global".to_string()));
    }

    #[test]
    fn test_keybindings_manager() {
        let mut manager = KeybindingsManager::new();

        manager.add(KeybindingDefinition::new("ctrl+c", "cancel", "Cancel"));
        manager.add(KeybindingDefinition::new("ctrl+v", "paste", "Paste"));

        assert_eq!(manager.len(), 2);
        assert!(manager.has_binding("ctrl+c"));
        assert!(manager.has_action("paste"));

        let keys = manager.get_keys_for_action("cancel").unwrap();
        assert!(keys.contains(&"ctrl+c".to_string()));
    }

    #[test]
    fn test_conflict_detection() {
        let mut manager = KeybindingsManager::new();

        manager.add(KeybindingDefinition::new("ctrl+c", "cancel", "Cancel"));
        manager.add(KeybindingDefinition::new("ctrl+c", "copy", "Copy"));

        let conflicts = manager.find_conflicts();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].key, "ctrl+c");
        assert_eq!(conflicts[0].actions.len(), 2);
    }

    #[test]
    fn test_default_keybindings() {
        let config = default_keybindings();
        assert!(!config.bindings.is_empty());

        // 检查一些关键绑定存在
        let has_ctrl_c = config.bindings.iter().any(|b| b.key == "ctrl+c" && b.action == "cancel");
        assert!(has_ctrl_c);
    }
}
