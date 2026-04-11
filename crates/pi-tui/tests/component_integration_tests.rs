//! TUI 组件集成测试
//!
//! 测试 TUI 组件的集成行为和公共 API

use pi_tui::{
    Component, Focusable, Container, Tui, OverlayOptions,
};
use pi_tui::tui::{OverlayAnchor, SizeValue, OverlayMargin};
use pi_tui::components::{
    editor::{Editor, EditorConfig, Selection},
    input::Input,
    select_list::{SelectList, SelectItem},
    settings_list::{SettingsList, SettingsCategory, SettingEntry, SettingValue},
    cancellable_loader::CancellableLoader,
};

// ============== Mock Terminal ==============

/// 模拟终端用于测试
struct MockTerminal {
    width: u16,
    height: u16,
    buffer: String,
    cursor_hidden: bool,
}

impl MockTerminal {
    fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            buffer: String::new(),
            cursor_hidden: true,
        }
    }
}

impl pi_tui::Terminal for MockTerminal {
    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }
    
    fn write(&mut self, data: &str) -> anyhow::Result<()> {
        self.buffer.push_str(data);
        Ok(())
    }
    
    fn flush(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn enable_raw_mode(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn disable_raw_mode(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn enter_alternate_screen(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn leave_alternate_screen(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn hide_cursor(&mut self) -> anyhow::Result<()> {
        self.cursor_hidden = true;
        Ok(())
    }
    
    fn show_cursor(&mut self) -> anyhow::Result<()> {
        self.cursor_hidden = false;
        Ok(())
    }
    
    fn move_cursor(&mut self, _row: u16, _col: u16) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn clear_line(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn clear_screen(&mut self) -> anyhow::Result<()> {
        self.buffer.clear();
        Ok(())
    }
}

// ============== Container 组件测试 ==============

/// 测试 Container 创建和基本操作
#[test]
fn test_container_creation() {
    let container = Container::new();
    
    // 初始状态
    assert!(container.is_empty());
    assert_eq!(container.len(), 0);
}

/// 测试 Container 添加和移除子组件
#[test]
fn test_container_add_remove_children() {
    let mut container = Container::new();
    
    // 创建简单的 mock 组件
    struct MockComponent;
    impl Component for MockComponent {
        fn render(&self, _width: u16) -> Vec<String> {
            vec!["mock".to_string()]
        }
        fn invalidate(&mut self) {}
    }
    
    // 添加子组件
    container.add_child(Box::new(MockComponent));
    assert_eq!(container.len(), 1);
    assert!(!container.is_empty());
    
    // 移除子组件
    let removed = container.remove_child(0);
    assert!(removed.is_some());
    assert!(container.is_empty());
    
    // 移除不存在的索引
    let removed = container.remove_child(0);
    assert!(removed.is_none());
}

/// 测试 Container 清空
#[test]
fn test_container_clear() {
    let mut container = Container::new();
    
    struct MockComponent;
    impl Component for MockComponent {
        fn render(&self, _width: u16) -> Vec<String> {
            vec!["mock".to_string()]
        }
        fn invalidate(&mut self) {}
    }
    
    // 添加多个子组件
    container.add_child(Box::new(MockComponent));
    container.add_child(Box::new(MockComponent));
    container.add_child(Box::new(MockComponent));
    
    assert_eq!(container.len(), 3);
    
    // 清空
    container.clear();
    assert!(container.is_empty());
    assert_eq!(container.len(), 0);
}

/// 测试 Container 渲染
#[test]
fn test_container_render() {
    let mut container = Container::new();
    
    struct MockComponent {
        lines: Vec<String>,
    }
    impl Component for MockComponent {
        fn render(&self, _width: u16) -> Vec<String> {
            self.lines.clone()
        }
        fn invalidate(&mut self) {}
    }
    
    container.add_child(Box::new(MockComponent {
        lines: vec!["Line 1".to_string()],
    }));
    container.add_child(Box::new(MockComponent {
        lines: vec!["Line 2".to_string()],
    }));
    
    let lines = container.render(80);
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "Line 1");
    assert_eq!(lines[1], "Line 2");
}

// ============== Editor 组件测试 ==============

/// 测试 Editor 创建和基本配置
#[test]
fn test_editor_creation() {
    let config = EditorConfig::default();
    let editor = Editor::new(config);
    
    assert!(editor.is_empty());
    assert_eq!(editor.line_count(), 1);
    assert_eq!(editor.cursor_position(), (0, 0));
}

/// 测试 Editor 多行编辑
#[test]
fn test_editor_multiline_editing() {
    let config = EditorConfig::default();
    let mut editor = Editor::new(config);
    
    // 插入多行文本
    editor.insert_text("Line 1");
    editor.new_line();
    editor.insert_text("Line 2");
    editor.new_line();
    editor.insert_text("Line 3");
    
    assert_eq!(editor.line_count(), 3);
    assert_eq!(editor.get_text(), "Line 1\nLine 2\nLine 3");
}

/// 测试 Editor 键盘处理
#[test]
fn test_editor_keyboard_handling() {
    let config = EditorConfig::default();
    let mut editor = Editor::new(config);
    
    // 模拟输入字符
    let handled = editor.handle_input("h");
    assert!(handled);
    assert_eq!(editor.get_text(), "h");
    
    // 模拟回车
    let handled = editor.handle_input("\r");
    assert!(handled);
    assert_eq!(editor.line_count(), 2);
    
    // 模拟退格
    editor.insert_text("test");
    let handled = editor.handle_input("\x7f");
    assert!(handled);
}

/// 测试 Editor 光标移动
#[test]
fn test_editor_cursor_movement() {
    let config = EditorConfig::default();
    let mut editor = Editor::new(config);
    
    editor.insert_text("Hello World");
    
    // 测试左移
    editor.move_left();
    assert_eq!(editor.cursor_position().1, 10);
    
    // 测试 Home
    editor.move_home();
    assert_eq!(editor.cursor_position().1, 0);
    
    // 测试 End
    editor.move_end();
    assert_eq!(editor.cursor_position().1, 11);
}

/// 测试 Editor 选择和删除
#[test]
fn test_editor_selection_and_deletion() {
    let config = EditorConfig::default();
    let mut editor = Editor::new(config);
    
    editor.insert_text("Hello World");
    
    // 全选
    editor.select_all();
    assert_eq!(editor.get_selected_text(), Some("Hello World".to_string()));
    
    // 删除选择
    editor.delete_char_before();
    assert!(editor.is_empty());
}

/// 测试 Editor 撤销重做
#[test]
fn test_editor_undo_redo() {
    let config = EditorConfig::default();
    let mut editor = Editor::new(config);
    
    // 插入文本
    editor.insert_text("Hello");
    assert_eq!(editor.get_text(), "Hello");
    
    // 撤销
    editor.undo();
    // 注意：当前实现撤销后应该回到空状态
    assert!(editor.is_empty());
}

/// 测试 Editor 只读模式
#[test]
fn test_editor_readonly_mode() {
    let config = EditorConfig {
        read_only: true,
        ..Default::default()
    };
    let mut editor = Editor::new(config);
    
    // 尝试在只读模式下插入
    editor.insert_text("Test");
    assert!(editor.is_empty());
    
    // 尝试在只读模式下删除
    editor.set_text("Initial");
    editor.delete_char_before();
    assert_eq!(editor.get_text(), "Initial");
}

/// 测试 Editor 最大行数限制
#[test]
fn test_editor_max_lines() {
    let config = EditorConfig {
        max_lines: Some(2),
        ..Default::default()
    };
    let mut editor = Editor::new(config);
    
    editor.insert_text("Line 1");
    editor.new_line();
    editor.insert_text("Line 2");
    editor.new_line(); // 应该被忽略
    editor.insert_text("Line 3"); // 应该被忽略
    
    assert_eq!(editor.line_count(), 2);
}

/// 测试 Editor Focusable trait
#[test]
fn test_editor_focusable() {
    let config = EditorConfig::default();
    let mut editor = Editor::new(config);
    
    assert!(!editor.focused());
    
    editor.set_focused(true);
    assert!(editor.focused());
    
    editor.set_focused(false);
    assert!(!editor.focused());
}

// ============== SettingsList 组件测试 ==============

/// 创建测试用的设置列表
fn create_test_settings() -> Vec<SettingsCategory> {
    vec![
        SettingsCategory::with_entries(
            "General",
            vec![
                SettingEntry::with_description(
                    "auto_save",
                    "Auto Save",
                    "Automatically save changes",
                    SettingValue::Boolean(true),
                ),
                SettingEntry::new(
                    "username",
                    "Username",
                    SettingValue::String("user".to_string()),
                ),
            ],
        ),
        SettingsCategory::with_entries(
            "Display",
            vec![
                SettingEntry::new(
                    "font_size",
                    "Font Size",
                    SettingValue::Number(14.0),
                ),
                SettingEntry::with_description(
                    "theme",
                    "Theme",
                    "Color theme for the interface",
                    SettingValue::Enum {
                        options: vec!["Light".to_string(), "Dark".to_string(), "System".to_string()],
                        selected: 1,
                    },
                ),
            ],
        ),
    ]
}

/// 测试 SettingsList 创建
#[test]
fn test_settings_list_creation() {
    let settings = create_test_settings();
    let list = SettingsList::new(settings);
    
    assert_eq!(list.total_count(), 6); // 2 categories + 4 entries
}

/// 测试 SettingsList 空列表
#[test]
fn test_settings_list_empty() {
    let list = SettingsList::empty();
    assert_eq!(list.total_count(), 0);
    assert!(list.selected_entry().is_none());
    
    let lines = list.render(40);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("No settings"));
}

/// 测试 SettingsList 导航
#[test]
fn test_settings_list_navigation() {
    let settings = create_test_settings();
    let mut list = SettingsList::new(settings);
    
    // 初始选中分类标题，select_next 应该跳到第一个设置项
    list.select_next();
    let entry = list.selected_entry();
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().1.key, "auto_save");
    
    // 继续导航
    list.select_next();
    assert_eq!(list.selected_entry().unwrap().1.key, "username");
    
    // 测试向上导航
    list.select_prev();
    assert_eq!(list.selected_entry().unwrap().1.key, "auto_save");
}

/// 测试 SettingsList 布尔值切换
#[test]
fn test_settings_list_boolean_toggle() {
    let settings = create_test_settings();
    let mut list = SettingsList::new(settings);
    
    list.select_next(); // 选中 auto_save
    
    let initial_value = list.selected_entry().unwrap().1.value.clone();
    assert_eq!(initial_value, SettingValue::Boolean(true));
    
    // 切换
    let changed = list.toggle_selected();
    assert!(changed);
    
    let new_value = list.selected_entry().unwrap().1.value.clone();
    assert_eq!(new_value, SettingValue::Boolean(false));
}

/// 测试 SettingsList 枚举切换
#[test]
fn test_settings_list_enum_toggle() {
    let settings = create_test_settings();
    let mut list = SettingsList::new(settings);
    
    // 导航到 theme 设置
    list.select_next(); // auto_save
    list.select_next(); // username
    list.select_next(); // font_size
    list.select_next(); // theme
    
    let entry = list.selected_entry().unwrap().1;
    if let SettingValue::Enum { selected, .. } = &entry.value {
        assert_eq!(*selected, 1); // Dark
    } else {
        panic!("Expected Enum value");
    }
    
    // 切换枚举
    list.toggle_selected();
    
    let entry = list.selected_entry().unwrap().1;
    if let SettingValue::Enum { selected, .. } = &entry.value {
        assert_eq!(*selected, 2); // System
    } else {
        panic!("Expected Enum value");
    }
}

/// 测试 SettingsList 数值调整
#[test]
fn test_settings_list_number_adjustment() {
    let settings = create_test_settings();
    let mut list = SettingsList::new(settings);
    
    // 导航到 font_size
    list.select_next(); // auto_save
    list.select_next(); // username
    list.select_next(); // font_size
    
    let initial = list.selected_entry().unwrap().1.value.clone();
    if let SettingValue::Number(n) = initial {
        assert_eq!(n, 14.0);
    } else {
        panic!("Expected Number value");
    }
    
    // 增加
    list.increment_number(2.0);
    
    let after_inc = list.selected_entry().unwrap().1.value.clone();
    if let SettingValue::Number(n) = after_inc {
        assert_eq!(n, 16.0);
    } else {
        panic!("Expected Number value");
    }
    
    // 减少
    list.decrement_number(1.0);
    
    let after_dec = list.selected_entry().unwrap().1.value.clone();
    if let SettingValue::Number(n) = after_dec {
        assert_eq!(n, 15.0);
    } else {
        panic!("Expected Number value");
    }
}

/// 测试 SettingsList 过滤
#[test]
fn test_settings_list_filter() {
    let settings = create_test_settings();
    let mut list = SettingsList::new(settings);
    
    // 过滤 "font"
    list.set_filter("font");
    
    // 应该只显示 font_size 设置
    list.select_next();
    let entry = list.selected_entry();
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().1.key, "font_size");
    
    // 清除过滤
    list.clear_filter();
    list.select_next();
    assert_eq!(list.selected_entry().unwrap().1.key, "auto_save");
}

/// 测试 SettingsList 键盘处理
#[test]
fn test_settings_list_keyboard_handling() {
    let settings = create_test_settings();
    let mut list = SettingsList::new(settings);
    
    // 测试向下导航
    let handled = list.handle_input("\x1b[B");
    assert!(handled);
    assert_eq!(list.selected_entry().unwrap().1.key, "auto_save");
    
    // 测试向上导航
    let handled = list.handle_input("\x1b[A");
    assert!(handled);
    
    // 测试 Enter 切换
    list.select_next();
    let handled = list.handle_input("\r");
    assert!(handled); // 切换布尔值
    
    // 测试 Tab 导航
    let handled = list.handle_input("\t");
    assert!(handled);
}

/// 测试 SettingsList Focusable trait
#[test]
fn test_settings_list_focusable() {
    let settings = create_test_settings();
    let mut list = SettingsList::new(settings);
    
    assert!(!list.focused());
    
    list.set_focused(true);
    assert!(list.focused());
    
    list.set_focused(false);
    assert!(!list.focused());
}

/// 测试 SettingsList 回调
#[test]
fn test_settings_list_callback() {
    let settings = create_test_settings();
    let mut list = SettingsList::new(settings);
    
    let changed_key = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let changed_key_clone = changed_key.clone();
    
    list.on_change(Box::new(move |key: &str, _value: &SettingValue| {
        *changed_key_clone.lock().unwrap() = key.to_string();
    }));
    
    list.select_next(); // 选中 auto_save
    list.toggle_selected();
    
    assert_eq!(*changed_key.lock().unwrap(), "auto_save");
}

// ============== CancellableLoader 组件测试 ==============

/// 测试 CancellableLoader 创建
#[test]
fn test_cancellable_loader_creation() {
    let loader = CancellableLoader::new("Loading...");
    assert_eq!(loader.message(), "Loading...");
    assert!(!loader.is_cancelled());
    assert!(loader.progress().is_none());
}

/// 测试 CancellableLoader 状态流转
#[test]
fn test_cancellable_loader_state_transitions() {
    let mut loader = CancellableLoader::new("Loading...");
    
    // 初始状态
    assert!(!loader.is_cancelled());
    assert_eq!(loader.current_frame_index(), 0);
    
    // 推进动画
    loader.tick();
    assert_eq!(loader.current_frame_index(), 1);
    
    // 模拟按下 Esc 键取消
    let handled = loader.handle_input("\x1b");
    assert!(handled);
    assert!(loader.is_cancelled());
}

/// 测试 CancellableLoader 进度设置
#[test]
fn test_cancellable_loader_progress() {
    let mut loader = CancellableLoader::new("Loading...");
    
    // 设置进度
    loader.set_progress(0.75);
    assert_eq!(loader.progress(), Some(0.75));
    
    // 使用 builder 模式设置进度
    let loader = CancellableLoader::new("Loading...").with_progress(0.45);
    assert_eq!(loader.progress(), Some(0.45));
    
    // 渲染验证
    let lines = loader.render(80);
    assert_eq!(lines.len(), 3); // 消息 + 进度条 + 取消提示
    assert!(lines[1].contains("45%"));
}

/// 测试 CancellableLoader 消息更新
#[test]
fn test_cancellable_loader_message_update() {
    let mut loader = CancellableLoader::new("Loading...");
    loader.set_message("Processing...");
    assert_eq!(loader.message(), "Processing...");
    
    let lines = loader.render(80);
    assert!(lines[0].contains("Processing..."));
}

/// 测试 CancellableLoader 取消提示
#[test]
fn test_cancellable_loader_cancel_hint() {
    let loader = CancellableLoader::new("Loading...")
        .with_cancel_hint("按 Esc 取消");
    
    let lines = loader.render(80);
    assert!(lines[1].contains("按 Esc 取消"));
}

/// 测试 CancellableLoader 取消回调
#[test]
fn test_cancellable_loader_cancel_callback() {
    let called = std::sync::Arc::new(std::sync::Mutex::new(false));
    let called_clone = called.clone();
    
    let mut loader = CancellableLoader::new("Loading...");
    loader.set_on_cancel(Box::new(move || {
        *called_clone.lock().unwrap() = true;
    }));
    
    loader.handle_input("\x1b");
    
    assert!(*called.lock().unwrap());
}

/// 测试 CancellableLoader 进度限制
#[test]
fn test_cancellable_loader_progress_clamping() {
    // 测试进度值超过 1.0 会被限制
    let loader = CancellableLoader::new("Loading...")
        .with_progress(1.5);
    assert_eq!(loader.progress(), Some(1.0));
    
    // 测试进度值小于 0.0 会被限制
    let loader = CancellableLoader::new("Loading...")
        .with_progress(-0.5);
    assert_eq!(loader.progress(), Some(0.0));
}

/// 测试 CancellableLoader 其他按键被忽略
#[test]
fn test_cancellable_loader_other_keys_ignored() {
    let mut loader = CancellableLoader::new("Loading...");
    
    // 其他按键不应触发取消
    let handled = loader.handle_input("a");
    assert!(!handled);
    assert!(!loader.is_cancelled());
    
    let handled = loader.handle_input("\r"); // Enter
    assert!(!handled);
    assert!(!loader.is_cancelled());
}

// ============== Input 组件测试 ==============

/// 测试 Input 创建
#[test]
fn test_input_creation() {
    let input = Input::new();
    assert!(input.is_empty());
    assert_eq!(input.len(), 0);
}

/// 测试 Input 带占位符
#[test]
fn test_input_with_placeholder() {
    let input = Input::with_placeholder("Enter text...");
    // 验证创建成功
    let lines = input.render(80);
    assert!(!lines.is_empty());
}

/// 测试 Input 文本编辑
#[test]
fn test_input_text_editing() {
    let mut input = Input::new();
    
    input.insert_char('h');
    input.insert_char('i');
    assert_eq!(input.text(), "hi");
    assert_eq!(input.len(), 2);
    
    input.delete_char_before();
    assert_eq!(input.text(), "h");
}

/// 测试 Input 光标移动
#[test]
fn test_input_cursor_movement() {
    let mut input = Input::new();
    input.insert_text("hello");
    
    input.move_left();
    // 光标应该在 'o' 之前
    
    input.move_home();
    // 光标应该在开头
    
    input.move_end();
    // 光标应该在末尾
}

/// 测试 Input 密码模式
#[test]
fn test_input_password_mode() {
    let input = Input::password();
    assert!(input.is_empty());
}

/// 测试 Input 最大长度限制
#[test]
fn test_input_max_length() {
    let mut input = Input::new();
    input.set_max_length(Some(5));
    input.insert_text("hello world");
    assert_eq!(input.text(), "hello");
}

/// 测试 Input 清空
#[test]
fn test_input_clear() {
    let mut input = Input::new();
    input.insert_text("hello");
    input.clear();
    assert!(input.is_empty());
    assert_eq!(input.len(), 0);
}

/// 测试 Input Focusable trait
#[test]
fn test_input_focusable() {
    let mut input = Input::new();
    
    assert!(!input.focused());
    
    input.set_focused(true);
    assert!(input.focused());
    
    input.set_focused(false);
    assert!(!input.focused());
}

// ============== SelectList 组件测试 ==============

/// 测试 SelectList 创建
#[test]
fn test_select_list_creation() {
    let items = vec![
        SelectItem::new("1", "Item 1"),
        SelectItem::new("2", "Item 2"),
        SelectItem::new("3", "Item 3"),
    ];
    let list = SelectList::new(items);
    
    assert_eq!(list.total_count(), 3);
    assert_eq!(list.filtered_count(), 3);
    assert!(list.selected().is_some());
}

/// 测试 SelectList 导航
#[test]
fn test_select_list_navigation() {
    let items = vec![
        SelectItem::new("1", "Item 1"),
        SelectItem::new("2", "Item 2"),
        SelectItem::new("3", "Item 3"),
    ];
    let mut list = SelectList::new(items);
    
    assert_eq!(list.selected().unwrap().value, "1");
    
    list.select_next();
    assert_eq!(list.selected().unwrap().value, "2");
    
    list.select_next();
    assert_eq!(list.selected().unwrap().value, "3");
    
    list.select_next();
    assert_eq!(list.selected().unwrap().value, "1"); // 循环
    
    list.select_prev();
    assert_eq!(list.selected().unwrap().value, "3");
}

/// 测试 SelectList 过滤
#[test]
fn test_select_list_filter() {
    let items = vec![
        SelectItem::new("apple", "Apple"),
        SelectItem::new("banana", "Banana"),
        SelectItem::new("apricot", "Apricot"),
    ];
    let mut list = SelectList::new(items);
    
    list.set_filter("ap");
    assert_eq!(list.filtered_count(), 2);
    
    list.set_filter("ban");
    assert_eq!(list.filtered_count(), 1);
    assert_eq!(list.selected().unwrap().value, "banana");
}

/// 测试 SelectList 带描述
#[test]
fn test_select_list_with_detail() {
    let item = SelectItem::with_detail("1", "Label", "This is a description");
    assert_eq!(item.label, "Label");
    assert_eq!(item.detail, Some("This is a description".to_string()));
}

/// 测试 SelectList 回调
#[test]
fn test_select_list_callback() {
    let items = vec![
        SelectItem::new("1", "Item 1"),
        SelectItem::new("2", "Item 2"),
    ];
    let mut list = SelectList::new(items);
    
    let selected = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let selected_clone = selected.clone();
    
    list.on_select(Box::new(move |item| {
        *selected_clone.lock().unwrap() = item.value.clone();
    }));
    
    list.confirm_selection();
    assert_eq!(*selected.lock().unwrap(), "1");
}

// ============== TUI 和覆盖层测试 ==============

/// 测试 TUI 创建
#[test]
fn test_tui_creation() {
    let terminal = Box::new(MockTerminal::new(80, 24));
    let mut tui = Tui::new(terminal);
    
    // 验证根容器可用
    let root = tui.root();
    assert!(root.is_empty());
}

/// 测试 TUI 批量更新模式
#[test]
fn test_tui_batch_mode() {
    let terminal = Box::new(MockTerminal::new(80, 24));
    let mut tui = Tui::new(terminal);
    
    // 开始批量更新
    tui.begin_batch();
    
    // 添加子组件
    struct MockComponent;
    impl Component for MockComponent {
        fn render(&self, _width: u16) -> Vec<String> {
            vec!["mock".to_string()]
        }
        fn invalidate(&mut self) {}
    }
    
    tui.root().add_child(Box::new(MockComponent));
    
    // 结束批量更新
    let result = tui.end_batch();
    assert!(result.is_ok());
}

/// 测试覆盖层选项
#[test]
fn test_overlay_options() {
    let options = OverlayOptions {
        width: Some(SizeValue::Absolute(40)),
        min_width: Some(20),
        max_height: Some(SizeValue::Percent(50.0)),
        anchor: Some(OverlayAnchor::Center),
        offset_x: Some(2),
        offset_y: Some(-1),
        row: None,
        col: None,
        margin: Some(OverlayMargin::uniform(2)),
        non_capturing: false,
    };
    
    // 验证选项创建成功
    assert!(options.width.is_some());
    assert!(options.anchor.is_some());
}

/// 测试覆盖层锚点
#[test]
fn test_overlay_anchor_variants() {
    let anchors = vec![
        OverlayAnchor::Center,
        OverlayAnchor::TopLeft,
        OverlayAnchor::TopRight,
        OverlayAnchor::BottomLeft,
        OverlayAnchor::BottomRight,
        OverlayAnchor::TopCenter,
        OverlayAnchor::BottomCenter,
        OverlayAnchor::LeftCenter,
        OverlayAnchor::RightCenter,
    ];
    
    // 验证默认锚点
    let default: OverlayAnchor = Default::default();
    assert_eq!(default, OverlayAnchor::Center);
    
    // 验证所有锚点都是唯一的
    let unique: std::collections::HashSet<_> = anchors.iter().map(std::mem::discriminant).collect();
    assert_eq!(unique.len(), anchors.len());
}

/// 测试 SizeValue
#[test]
fn test_size_value() {
    // 测试创建 SizeValue
    let abs = SizeValue::Absolute(10);
    let pct = SizeValue::Percent(50.0);
    
    // 验证创建成功
    match abs {
        SizeValue::Absolute(v) => assert_eq!(v, 10),
        _ => panic!("Expected Absolute"),
    }
    
    match pct {
        SizeValue::Percent(p) => assert_eq!(p, 50.0),
        _ => panic!("Expected Percent"),
    }
    
    // 测试 From<u16>
    let val: SizeValue = 20.into();
    match val {
        SizeValue::Absolute(v) => assert_eq!(v, 20),
        _ => panic!("Expected Absolute from u16"),
    }
}

/// 测试覆盖层边距
#[test]
fn test_overlay_margin() {
    let margin = OverlayMargin::uniform(4);
    assert_eq!(margin.top, 4);
    assert_eq!(margin.right, 4);
    assert_eq!(margin.bottom, 4);
    assert_eq!(margin.left, 4);
    
    let default: OverlayMargin = Default::default();
    assert_eq!(default.top, 0);
    assert_eq!(default.right, 0);
}

// ============== 多组件组合测试 ==============

/// 测试多组件组合渲染
#[test]
fn test_multi_component_composition() {
    let mut container = Container::new();
    
    // 添加不同类型的组件
    let editor = Editor::new(EditorConfig::default());
    let input = Input::with_placeholder("Search...");
    
    container.add_child(Box::new(editor));
    container.add_child(Box::new(input));
    
    assert_eq!(container.len(), 2);
    
    // 渲染
    let lines = container.render(80);
    assert!(!lines.is_empty());
}

/// 测试焦点管理和切换
#[test]
fn test_focus_management() {
    let mut editor1 = Editor::new(EditorConfig::default());
    let mut editor2 = Editor::new(EditorConfig::default());
    
    // 初始状态
    assert!(!editor1.focused());
    assert!(!editor2.focused());
    
    // 设置焦点
    editor1.set_focused(true);
    assert!(editor1.focused());
    
    // 切换到另一个编辑器
    editor1.set_focused(false);
    editor2.set_focused(true);
    assert!(!editor1.focused());
    assert!(editor2.focused());
}

// ============== SettingValue 测试 ==============

/// 测试 SettingValue 显示文本
#[test]
fn test_setting_value_display() {
    assert_eq!(SettingValue::Boolean(true).display_text(), "✓");
    assert_eq!(SettingValue::Boolean(false).display_text(), "✗");
    assert_eq!(SettingValue::String("test".to_string()).display_text(), "test");
    assert_eq!(SettingValue::Number(3.15).display_text(), "3.15");
    assert_eq!(
        SettingValue::Enum {
            options: vec!["A".to_string(), "B".to_string()],
            selected: 0
        }.display_text(),
        "A"
    );
}

/// 测试 SettingValue 可编辑性
#[test]
fn test_setting_value_editable() {
    assert!(SettingValue::Boolean(true).is_editable());
    assert!(SettingValue::Number(1.0).is_editable());
    assert!(SettingValue::Enum { options: vec![], selected: 0 }.is_editable());
    assert!(!SettingValue::String("test".to_string()).is_editable());
}

// ============== Selection 测试 ==============

/// 测试 Selection 创建和规范化
#[test]
fn test_selection_creation() {
    let sel = Selection::new(0, 0, 1, 5);
    assert_eq!(sel.start_row, 0);
    assert_eq!(sel.end_row, 1);
    
    // 测试规范化
    let sel = Selection::new(1, 5, 0, 0);
    let normalized = sel.normalized();
    assert_eq!(normalized.start_row, 0);
    assert_eq!(normalized.end_row, 1);
}

/// 测试 Selection 空检查
#[test]
fn test_selection_is_empty() {
    let sel = Selection::new(0, 0, 0, 0);
    assert!(sel.is_empty());
    
    let sel = Selection::new(0, 0, 0, 5);
    assert!(!sel.is_empty());
}
