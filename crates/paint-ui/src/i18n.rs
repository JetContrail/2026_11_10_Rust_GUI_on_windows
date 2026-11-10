//! User-visible UI strings, grouped per language.
//!
//! The UI is Japanese only for now (Issue #19 / #29). To add a language,
//! add a [`Language`] variant and a [`Strings`] table for it; the compiler
//! then reports any missing translation.

/// UI display language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    Japanese,
}

impl Language {
    pub const ALL: [Self; 1] = [Self::Japanese];

    pub const fn strings(self) -> &'static Strings {
        match self {
            Self::Japanese => &JA,
        }
    }
}

/// Strings of the language currently used by the UI.
pub(crate) const fn tr() -> &'static Strings {
    Language::Japanese.strings()
}

/// All UI strings of one language.
#[derive(Debug)]
pub struct Strings {
    pub ready: &'static str,
    pub unsaved_title: &'static str,
    pub unsaved_question: &'static str,
    pub save: &'static str,
    pub dont_save: &'static str,
    pub cancel: &'static str,
    pub untitled: &'static str,
    pub menu_file: &'static str,
    pub open_image: &'static str,
    pub open_project: &'static str,
    pub save_shortcut: &'static str,
    pub save_as: &'static str,
    pub export_png: &'static str,
    pub export_transparent_png: &'static str,
    pub exit: &'static str,
    pub menu_edit: &'static str,
    pub undo_shortcut: &'static str,
    pub redo_shortcut: &'static str,
    pub delete_selection: &'static str,
    pub menu_view: &'static str,
    pub zoom_in: &'static str,
    pub zoom_out: &'static str,
    pub tools: &'static str,
    pub pen_width: &'static str,
    pub eraser_size: &'static str,
    pub color: &'static str,
    pub layers: &'static str,
    pub layer_visible: &'static str,
    pub layer_locked_suffix: &'static str,
    pub layer_editing_suffix: &'static str,
    pub undo_count: &'static str,
    pub redo_count: &'static str,
    pub image_loaded: &'static str,
    pub project_opened: &'static str,
    pub saved: &'static str,
    pub png_exported: &'static str,
    pub selection_deleted: &'static str,
    pub undone: &'static str,
    pub redone: &'static str,
    pub tool_pen: &'static str,
    pub tool_line: &'static str,
    pub tool_eraser: &'static str,
    pub tool_select: &'static str,
    pub tool_text: &'static str,
    pub filter_images: &'static str,
    pub filter_project: &'static str,
    pub filter_png: &'static str,
    pub font_preview_sample: &'static str,
    pub text_size: &'static str,
    pub font_label: &'static str,
    pub choose_font: &'static str,
    pub text_hint: &'static str,
    pub font_window_title: &'static str,
    pub search: &'static str,
    pub no_fonts: &'static str,
    pub preview_label: &'static str,
}

const JA: Strings = Strings {
    ready: "準備完了",
    unsaved_title: "未保存の変更があります",
    unsaved_question: "変更を保存しますか?",
    save: "保存",
    dont_save: "保存しない",
    cancel: "キャンセル",
    untitled: "無題",
    menu_file: "ファイル",
    open_image: "画像を開く… (Ctrl+O)",
    open_project: "プロジェクトを開く…",
    save_shortcut: "保存 (Ctrl+S)",
    save_as: "名前を付けて保存…",
    export_png: "PNG 書き出し (表示中レイヤー)…",
    export_transparent_png: "透過 PNG 書き出し (描画レイヤーのみ)…",
    exit: "終了",
    menu_edit: "編集",
    undo_shortcut: "元に戻す (Ctrl+Z)",
    redo_shortcut: "やり直し (Ctrl+Y)",
    delete_selection: "選択範囲を削除 (Delete)",
    menu_view: "表示",
    zoom_in: "拡大",
    zoom_out: "縮小",
    tools: "ツール",
    pen_width: "線幅",
    eraser_size: "消しゴムサイズ",
    color: "色",
    layers: "レイヤー",
    layer_visible: "表示",
    layer_locked_suffix: "(編集禁止)",
    layer_editing_suffix: "(編集中)",
    undo_count: "元に戻す",
    redo_count: "やり直し",
    image_loaded: "画像を読み込みました",
    project_opened: "プロジェクトを開きました",
    saved: "保存しました",
    png_exported: "PNG を書き出しました",
    selection_deleted: "選択範囲を削除しました",
    undone: "元に戻しました",
    redone: "やり直しました",
    tool_pen: "ペン",
    tool_line: "直線",
    tool_eraser: "消しゴム",
    tool_select: "範囲選択",
    tool_text: "テキスト",
    filter_images: "画像 (PNG / JPEG / BMP)",
    filter_project: "プロジェクト (*.paint)",
    filter_png: "PNG 画像",
    font_preview_sample: "Aa Bb あいう 漢字 123",
    text_size: "文字サイズ",
    font_label: "フォント",
    choose_font: "フォント選択…",
    text_hint: "文字を入力 (Ctrl+Enter: 確定 / Esc: 取消)",
    font_window_title: "フォントの選択",
    search: "検索",
    no_fonts: "インストール済みのフォントが見つかりません",
    preview_label: "プレビュー",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_nonempty_strings() {
        for lang in Language::ALL {
            let s = format!("{:?}", lang.strings());
            assert!(!s.contains(": \"\""), "{lang:?} has an empty string");
        }
    }
}
