//! Completed-turn summaries and a resizable diff panel inside the development viewport.
use crate::development::{
    Turn,
    review::{self, FileReview, Line, LineKind},
};
use eframe::egui;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

struct FileView {
    file: FileReview,
    rows: Option<Vec<Line>>,
    counts: (usize, usize),
}

struct ReviewView {
    files: Vec<FileView>,
    counts: (usize, usize),
}

#[derive(Clone)]
pub(super) struct Selection {
    key: egui::Id,
    view: Arc<ReviewView>,
    index: usize,
}

#[derive(Default)]
pub(super) struct ReviewUi {
    selection: Option<Selection>,
    owner: Option<(PathBuf, u64)>,
}

impl ReviewUi {
    pub fn has_selection(&self) -> bool {
        self.selection.is_some()
    }
    pub fn select(&mut self, selection: Selection, owner: Option<(PathBuf, u64)>) {
        self.selection = Some(selection);
        self.owner = owner;
    }
    pub fn clear(&mut self) {
        self.selection = None;
        self.owner = None;
    }

    pub fn panel(
        &mut self,
        ui: &mut egui::Ui,
        owner: Option<&(PathBuf, u64)>,
    ) -> Option<egui::Rect> {
        if self.owner.as_ref() != owner {
            self.clear();
        }
        self.selection.as_ref()?;
        let maximum = (ui.available_width() - 600.).max(280.);
        let mut open = true;
        let panel = egui::SidePanel::right("development-file-review-panel")
            .resizable(true)
            .default_width(480.)
            .width_range(280. ..=maximum)
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(247, 249, 252))
                    .inner_margin(14),
            )
            .show_inside(ui, |ui| {
                open = self.contents(ui);
            });
        if !open {
            self.clear();
        }
        Some(panel.response.rect)
    }

    fn contents(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(selection) = &mut self.selection else {
            return true;
        };
        let mut open = true;
        ui.horizontal(|ui| {
            ui.heading("文件差异");
            counts(ui, selection.view.counts);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("关闭").clicked() {
                    open = false;
                }
            });
        });
        ui.small("拖动左侧分隔线调宽 · 代码自动换行");
        let file = &selection.view.files[selection.index].file;
        egui::ComboBox::from_id_salt("review-file-selector")
            .selected_text(&file.path)
            .truncate()
            .width((ui.available_width() - 32.).max(32.))
            .show_ui(ui, |ui| {
                for (index, file) in selection.view.files.iter().enumerate() {
                    ui.selectable_value(&mut selection.index, index, &file.file.path);
                }
            });
        ui.push_id((selection.key, selection.index), |ui| {
            file_view(ui, &selection.view.files[selection.index]);
        });
        open
    }
}

pub(super) fn show(ui: &mut egui::Ui, turn: &Turn) -> Option<Selection> {
    let id = ui.id().with("completed-review");
    let view = ui.data_mut(|data| {
        data.get_temp_mut_or_insert_with(id, || {
            let files = turn
                .review
                .clone()
                .unwrap_or_else(|| review::recorded_files(turn));
            let files: Vec<_> = files
                .into_iter()
                .map(|file| {
                    let rows = file.lines();
                    let counts = rows.as_ref().map_or_else(
                        || file.counts(),
                        |rows| {
                            (
                                rows.iter()
                                    .filter(|row| row.kind == LineKind::Added)
                                    .count(),
                                rows.iter()
                                    .filter(|row| row.kind == LineKind::Removed)
                                    .count(),
                            )
                        },
                    );
                    FileView { file, rows, counts }
                })
                .collect();
            let counts = files.iter().fold((0, 0), |(added, removed), file| {
                (added + file.counts.0, removed + file.counts.1)
            });
            Arc::new(ReviewView { files, counts })
        })
        .clone()
    });
    if view.files.is_empty() {
        ui.weak("本轮没有记录到文件变更。");
        return None;
    }
    let selected_id = id.with("selected");
    let mut selected = ui.data(|data| data.get_temp::<usize>(selected_id));
    let mut clicked = None;
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(247, 249, 252))
        .stroke(egui::Stroke::new(1_f32, egui::Color32::from_gray(220)))
        .corner_radius(7)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("{} 个文件有改动", view.files.len()));
                ui.label(format!("共 {} 行差异", view.counts.0 + view.counts.1));
                counts(ui, view.counts);
            });
            ui.weak("点击文件名，在右侧查看全文与红绿差异。");
            let row_height = (ui.text_style_height(&egui::TextStyle::Button)
                + 2. * ui.spacing().button_padding.y)
                .max(ui.spacing().interact_size.y);
            egui::ScrollArea::vertical()
                .id_salt(id.with("files"))
                .max_height(160.)
                .min_scrolled_height(
                    ((row_height + ui.spacing().item_spacing.y) * view.files.len().min(5) as f32)
                        .min(160.),
                )
                .auto_shrink([false, true])
                .show_rows(ui, row_height, view.files.len(), |ui, rows| {
                    for index in rows {
                        let file = &view.files[index];
                        ui.horizontal(|ui| {
                            ui.weak(match file.file.kind.as_str() {
                                "add" => "新增",
                                "delete" => "删除",
                                "rename" => "重命名",
                                _ => "修改",
                            });
                            let width = (ui.available_width() - 110.).max(24.);
                            let text = egui::WidgetText::from(file.file.path.as_str()).into_galley(
                                ui,
                                Some(egui::TextWrapMode::Truncate),
                                width,
                                egui::TextStyle::Button,
                            );
                            if ui
                                .selectable_label(selected == Some(index), text)
                                .on_hover_text(&file.file.path)
                                .clicked()
                            {
                                selected = Some(index);
                                clicked = Some(Selection {
                                    key: id,
                                    view: view.clone(),
                                    index,
                                });
                            }
                            counts(ui, file.counts);
                        });
                    }
                });
        });
    ui.data_mut(|data| {
        if let Some(index) = selected {
            data.insert_temp(selected_id, index);
        } else {
            data.remove::<usize>(selected_id);
        }
    });
    clicked
}

fn counts(ui: &mut egui::Ui, (added, removed): (usize, usize)) {
    ui.colored_label(egui::Color32::from_rgb(25, 125, 65), format!("+{added}"));
    ui.colored_label(egui::Color32::from_rgb(190, 55, 55), format!("−{removed}"));
}

fn file_view(ui: &mut egui::Ui, view: &FileView) {
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(&view.file.path).strong()).truncate())
            .on_hover_text(&view.file.path);
        if let Some(content) = &view.file.content
            && ui.small_button("复制全文").clicked()
        {
            ui.ctx().copy_text(content.clone());
        }
        if view.rows.is_none() && ui.small_button("复制差异").clicked() {
            ui.ctx().copy_text(view.file.patches.join("\n"));
        }
    });
    if !view.file.notice.is_empty() {
        ui.weak(&view.file.notice);
    }
    if let Some(rows) = &view.rows {
        ui.weak("结束时的文件快照 · 左列原行号 / 右列新行号");
        if rows.is_empty() {
            ui.weak("空文件");
        }
    }
    let width = (ui.available_width()
        - ui.spacing().scroll.allocated_width()
        - ui.spacing().scroll.bar_width
        - 4.)
        .floor()
        .max(1.);
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let scale = ui.ctx().pixels_per_point();
    let cache_id = ui.id().with("wrapped-review-code");
    let cached = ui
        .data(|data| data.get_temp::<Arc<WrappedCode>>(cache_id))
        .filter(|cache| cache.width == width && cache.font == font && cache.scale == scale);
    let code = cached.unwrap_or_else(|| {
        let fallback;
        let rows = if let Some(rows) = &view.rows {
            rows.as_slice()
        } else {
            // Legacy/mismatched evidence uses the same wrapping and scrolling as
            // verified files, while keeping the snapshot and recorded diff distinct.
            let mut rows = Vec::new();
            if let Some(content) = &view.file.content {
                rows.push(Line {
                    old: None,
                    new: None,
                    kind: LineKind::Context,
                    text: "── 结束时的完整文件 ──".into(),
                });
                rows.extend(content.lines().enumerate().map(|(index, text)| Line {
                    old: None,
                    new: Some(index + 1),
                    kind: LineKind::Context,
                    text: text.into(),
                }));
            }
            rows.push(Line {
                old: None,
                new: None,
                kind: LineKind::Context,
                text: "── 已记录的差异（无法核对全文时按工具记录计数）──".into(),
            });
            rows.extend(
                view.file
                    .patches
                    .iter()
                    .flat_map(|patch| patch.lines())
                    .map(|text| {
                        let (kind, text) = if text.starts_with('+') && !text.starts_with("+++") {
                            (LineKind::Added, &text[1..])
                        } else if text.starts_with('-') && !text.starts_with("---") {
                            (LineKind::Removed, &text[1..])
                        } else {
                            (LineKind::Context, text)
                        };
                        Line {
                            old: None,
                            new: None,
                            kind,
                            text: text.into(),
                        }
                    }),
            );
            fallback = rows;
            &fallback
        };
        let code = Arc::new(WrappedCode {
            width,
            scale,
            lines: wrap_rows(ui, rows, width),
            font,
        });
        // Replace the previous width's cache instead of accumulating a full copy
        // for every intermediate divider position.
        ui.data_mut(|data| data.insert_temp(cache_id, code.clone()));
        code
    });
    egui::ScrollArea::vertical()
        .id_salt("full-file")
        .max_height(ui.available_height().max(1.))
        .min_scrolled_height(0.)
        .auto_shrink([false, false])
        .show_rows(
            ui,
            ui.text_style_height(&egui::TextStyle::Monospace),
            code.lines.len(),
            |ui, range| {
                let text = code.lines[range.clone()].join("\n");
                ui.push_id((range.start, width.to_bits(), scale.to_bits()), |ui| {
                    super::development::output_text(ui, &text, true, true);
                });
            },
        );
}

struct WrappedCode {
    width: f32,
    scale: f32,
    font: egui::FontId,
    lines: Vec<String>,
}

fn wrap_rows(ui: &egui::Ui, rows: &[Line], width: f32) -> Vec<String> {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    ui.fonts(|fonts| {
        let mut glyphs = HashMap::new();
        let mut advance = |ch| {
            *glyphs
                .entry(ch)
                .or_insert_with(|| fonts.glyph_width(&font, ch))
        };
        let mut result = Vec::new();
        for row in rows {
            let marker = match row.kind {
                LineKind::Context => ' ',
                LineKind::Added => '+',
                LineKind::Removed => '-',
            };
            let old = row.old.map_or_else(String::new, |n| n.to_string());
            let new = row.new.map_or_else(String::new, |n| n.to_string());
            let (old_width, new_width) = (old.len().max(5), new.len().max(5));
            let prefix = format!("{marker} {old:>old_width$} {new:>new_width$} │ ");
            let continuation = format!("{marker} {:>old_width$} {:>new_width$} │ ", "", "");
            let budget = (width - prefix.chars().map(&mut advance).sum::<f32>()).max(1.);
            let mut prefix = prefix.as_str();
            let mut body = String::new();
            let mut used = 0.;
            for ch in row.text.chars() {
                // Expand tabs for display only; copying the full file still
                // returns the untouched snapshot, including tabs/newlines.
                let count = if ch == '\t' { 4 } else { 1 };
                let ch = if ch == '\t' { ' ' } else { ch };
                for _ in 0..count {
                    let glyph = advance(ch);
                    if !body.is_empty() && used + glyph > budget {
                        result.push(format!("{prefix}{body}"));
                        prefix = &continuation;
                        body.clear();
                        used = 0.;
                    }
                    body.push(ch);
                    used += glyph;
                }
            }
            result.push(format!("{prefix}{body}"));
        }
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_reflows_unicode_and_identifiers_without_changing_line_numbers_or_counts() {
        let ctx = egui::Context::default();
        super::super::style::install(&ctx);
        let text = format!(
            "\t{}{}🙂END",
            "identifier_without_breaks".repeat(8),
            "中文内容".repeat(12)
        );
        let rows = [
            Line {
                old: Some(10),
                new: Some(10),
                kind: LineKind::Context,
                text: text.clone(),
            },
            Line {
                old: Some(11),
                new: None,
                kind: LineKind::Removed,
                text: text.clone(),
            },
            Line {
                old: None,
                new: Some(11),
                kind: LineKind::Added,
                text: text.clone(),
            },
        ];
        let mut wrapped = Vec::new();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for width in [220., 320., 700.] {
                    let lines = wrap_rows(ui, &rows, width);
                    let font = egui::TextStyle::Monospace.resolve(ui.style());
                    for line in &lines {
                        let size = ui.fonts(|fonts| {
                            fonts
                                .layout_no_wrap(line.clone(), font.clone(), egui::Color32::BLACK)
                                .size()
                        });
                        assert!(size.x <= width + 0.5, "{} exceeds {width}: {line}", size.x);
                    }
                    for marker in [' ', '-', '+'] {
                        let group: Vec<_> = lines
                            .iter()
                            .filter(|line| line.starts_with(marker))
                            .collect();
                        assert!(group.len() > 1);
                        let restored = group
                            .iter()
                            .map(|line| line.split_once(" │ ").unwrap().1)
                            .collect::<String>();
                        assert_eq!(restored, text.replace('\t', "    "));
                        assert!(
                            group
                                .iter()
                                .skip(1)
                                .all(|line| line.split_once(" │ ").unwrap().0[1..]
                                    .trim()
                                    .is_empty()),
                            "continuation rows must not invent source line numbers"
                        );
                    }
                    wrapped.push(lines.len());
                }
            });
        });
        assert!(wrapped[0] > wrapped[1] && wrapped[1] > wrapped[2]);
        assert_eq!(
            rows.iter()
                .filter(|line| line.kind == LineKind::Added)
                .count(),
            1
        );
        assert_eq!(
            rows.iter()
                .filter(|line| line.kind == LineKind::Removed)
                .count(),
            1
        );
    }

    #[test]
    fn wrapped_file_cache_tracks_width_and_full_copy_keeps_original_tabs_and_newlines() {
        let ctx = egui::Context::default();
        super::super::style::install(&ctx);
        let text = format!("\t{}中文🙂END\n", "long_identifier".repeat(90));
        let view = FileView {
            file: FileReview {
                path: "a.rs".into(),
                content: Some(text.clone()),
                ..Default::default()
            },
            rows: Some(vec![Line {
                old: None,
                new: Some(1),
                kind: LineKind::Added,
                text: text.trim_end_matches('\n').into(),
            }]),
            counts: (1, 0),
        };
        let frame = |width, events| {
            let mut cache = None;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 320.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let bounds = ui.available_rect_before_wrap();
                        file_view(ui, &view);
                        assert!(ui.min_rect().right() <= bounds.right() + 1.);
                        cache = ui.data(|data| {
                            data.get_temp::<Arc<WrappedCode>>(ui.id().with("wrapped-review-code"))
                        });
                    });
                },
            );
            for shape in &output.shapes {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text.contains(" │ ")
                {
                    assert!(
                        text.pos.x + text.galley.size().x <= shape.clip_rect.right() + 1.,
                        "wrapped code must stay within the panel"
                    );
                    assert!(
                        !text.galley.job.text.contains("END"),
                        "a long logical line must still draw only visible wrapped rows"
                    );
                }
            }
            (output, cache.unwrap())
        };
        let (_, narrow) = frame(280., vec![]);
        let (_, same) = frame(280., vec![]);
        assert!(
            Arc::ptr_eq(&narrow, &same),
            "unchanged width must reuse cached wrap boundaries"
        );
        let (output, wide) = frame(620., vec![]);
        assert!(!Arc::ptr_eq(&narrow, &wide));
        assert!(wide.lines.len() < narrow.lines.len());
        let copy = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "复制全文"
                {
                    Some(text.pos + text.galley.size() / 2.)
                } else {
                    None
                }
            })
            .unwrap();
        let button = |pressed| egui::Event::PointerButton {
            pos: copy,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = frame(620., vec![egui::Event::PointerMoved(copy), button(true)]);
        let (output, _) = frame(620., vec![button(false)]);
        assert!(output.platform_output.commands.iter().any(
            |command| matches!(command, egui::OutputCommand::CopyText(value) if value == &text)
        ));
        assert_eq!(view.file.content.as_deref(), Some(text.as_str()));
    }

    #[test]
    fn file_click_opens_resizable_panel_in_the_same_viewport_and_keeps_editor_visible() {
        let ctx = egui::Context::default();
        super::super::style::install(&ctx);
        ctx.set_embed_viewports(false);
        egui::Context::set_immediate_viewport_renderer(|_, _| {
            panic!("file review must not create another viewport")
        });
        let file = FileReview {
            path: "a.rs".into(),
            content: Some(format!(
                "BEFORE\nADDED\nAFTER\nOUTSIDE-HUNK\n{}",
                "untouched\n".repeat(80)
            )),
            patches: vec!["@@ -1,3 +1,3 @@\n BEFORE\n-REMOVED\n+ADDED\n AFTER\n".into()],
            ..Default::default()
        };
        let turn = Turn {
            review: Some(vec![
                file,
                FileReview {
                    path: "b.rs".into(),
                    content: Some("SECOND-FILE\n".into()),
                    patches: vec!["@@ -0,0 +1 @@\n+SECOND-FILE\n".into()],
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        let owner = Some((PathBuf::from("/fixture"), 1));
        let frame = |viewer: &mut ReviewUi, events, size, owner: Option<&(PathBuf, u64)>| {
            let mut selected = None;
            let mut pane = None;
            let mut editor = egui::Rect::NOTHING;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        pane = viewer.panel(ui, owner);
                        let bounds = ui.available_rect_before_wrap();
                        selected = show(ui, &turn);
                        ui.scope_builder(
                            egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
                                egui::pos2(bounds.left(), bounds.bottom() - 80.),
                                bounds.max,
                            )),
                            |ui| {
                                let mut draft = "DRAFT".to_owned();
                                editor = ui
                                    .add(
                                        egui::TextEdit::multiline(&mut draft)
                                            .desired_rows(3)
                                            .desired_width(f32::INFINITY),
                                    )
                                    .rect;
                            },
                        );
                        assert!(
                            bounds.contains_rect(editor),
                            "the editor must remain inside the left column"
                        );
                    });
                },
            );
            assert!(
                output
                    .viewport_output
                    .keys()
                    .all(|id| *id == egui::ViewportId::ROOT)
            );
            (output, selected, pane, editor)
        };
        let size = egui::vec2(1440., 740.);
        let mut viewer = ReviewUi::default();
        let _ = frame(&mut viewer, vec![], size, owner.as_ref());
        let (output, _, _, _) = frame(&mut viewer, vec![], size, owner.as_ref());
        for path in ["a.rs", "b.rs"] {
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == path
                    && shape.clip_rect.contains_rect(text.galley.rect.translate(text.pos.to_vec2())))),
                "each file row must reserve its full selectable-button height");
        }
        let point = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "a.rs"
                {
                    Some(text.pos + text.galley.size() / 2.)
                } else {
                    None
                }
            })
            .unwrap();
        let click = |point, pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let _ = frame(
            &mut viewer,
            vec![egui::Event::PointerMoved(point), click(point, true)],
            size,
            owner.as_ref(),
        );
        let (_, selection, _, _) =
            frame(&mut viewer, vec![click(point, false)], size, owner.as_ref());
        let selection = selection.expect("clicking a filename selects its full-file review");
        viewer.select(selection.clone(), owner.clone());
        let _ = frame(&mut viewer, vec![], size, owner.as_ref());
        let (output, _, pane, editor) = frame(&mut viewer, vec![], size, owner.as_ref());
        let pane = pane.unwrap();
        assert!(editor.right() < pane.left());
        assert!(pane.height() >= 700.);
        let body = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text.contains("OUTSIDE-HUNK")
                {
                    assert!(
                        shape.clip_rect.left() >= pane.left(),
                        "full-file text belongs only to the right panel"
                    );
                    Some(&text.galley.job)
                } else {
                    None
                }
            })
            .expect("the right panel contains unchanged full-file context");
        assert!(
            body.sections
                .iter()
                .any(|s| s.format.color == egui::Color32::from_rgb(25, 125, 65)
                    && body.text[s.byte_range.clone()].contains("ADDED"))
        );
        assert!(body.sections.iter().any(|s| s.format.background
            == egui::Color32::from_rgb(253, 232, 232)
            && body.text[s.byte_range.clone()].contains("REMOVED")));
        assert!(body.text.contains("2       │ REMOVED"));
        assert!(body.text.contains("2 │ ADDED"));
        let handle = egui::pos2(pane.left(), pane.center().y);
        let _ = frame(
            &mut viewer,
            vec![egui::Event::PointerMoved(handle), click(handle, true)],
            size,
            owner.as_ref(),
        );
        let dragged = handle - egui::vec2(260., 0.);
        let _ = frame(
            &mut viewer,
            vec![egui::Event::PointerMoved(dragged)],
            size,
            owner.as_ref(),
        );
        let _ = frame(
            &mut viewer,
            vec![click(dragged, false)],
            size,
            owner.as_ref(),
        );
        let (_, _, resized, _) = frame(&mut viewer, vec![], size, owner.as_ref());
        assert!(
            resized.unwrap().width() > 680.,
            "the divider must resize the panel"
        );
        let (output, _, _, _) = frame(&mut viewer, vec![], size, owner.as_ref());
        let close = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape
                    && text.galley.job.text == "关闭"
                {
                    Some(text.pos + text.galley.size() / 2.)
                } else {
                    None
                }
            })
            .unwrap();
        let _ = frame(
            &mut viewer,
            vec![egui::Event::PointerMoved(close), click(close, true)],
            size,
            owner.as_ref(),
        );
        let _ = frame(&mut viewer, vec![click(close, false)], size, owner.as_ref());
        assert!(!viewer.has_selection());
        viewer.select(selection.clone(), owner.clone());
        let _ = frame(&mut viewer, vec![], egui::vec2(1040., 500.), owner.as_ref());
        assert!(viewer.has_selection());
        let other = Some((PathBuf::from("/other-project"), 1));
        let (_, _, pane, _) = frame(&mut viewer, vec![], size, other.as_ref());
        assert!(
            pane.is_none() && !viewer.has_selection(),
            "equal task IDs in different projects must not reuse the file view"
        );
        egui::Context::set_immediate_viewport_renderer(|_, _| {});
    }

    #[test]
    fn full_file_side_view_virtualizes_large_files_and_bounds_long_paths() {
        let ctx = egui::Context::default();
        let content = (0..6000)
            .map(|n| format!("FILE-LINE-{n}\n"))
            .collect::<String>();
        let file = FileReview {
            path: "long_directory/".repeat(70),
            content: Some(content),
            patches: vec!["@@ -1 +1 @@\n-old\n+FILE-LINE-0\n".into()],
            ..Default::default()
        };
        let rows = file.lines();
        let mut viewer = ReviewUi::default();
        viewer.select(
            Selection {
                key: egui::Id::new("large-file"),
                index: 0,
                view: Arc::new(ReviewView {
                    files: vec![FileView {
                        file,
                        rows,
                        counts: (1, 1),
                    }],
                    counts: (1, 1),
                }),
            },
            None,
        );
        for _ in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(420., 700.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let bounds = ui.available_rect_before_wrap();
                        viewer.contents(ui);
                        assert!(ui.min_rect().right() <= bounds.right() + 1.);
                        assert!(ui.min_rect().bottom() <= bounds.bottom() + 1.);
                    });
                },
            );
            for shape in output.shapes {
                if let egui::Shape::Text(text) = shape.shape {
                    assert!(!text.galley.job.text.contains("FILE-LINE-5999"));
                    assert!(text.galley.job.text.lines().count() < 100);
                    assert!(!text.galley.job.text.contains("use of ScrollArea ID"));
                }
            }
        }
    }
}
