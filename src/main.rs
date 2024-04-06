use chrono::NaiveDateTime;
use color_eyre::config::HookBuilder;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{
        self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
    ExecutableCommand,
};
use ratatui::{prelude::*, style::palette::tailwind, widgets::*};
use serde::{Deserialize, Serialize};
use std::{error::Error, io, io::stdout};
use uuid::Uuid;

const TODO_HEADER_BG: Color = tailwind::BLUE.c950;
const NORMAL_ROW_COLOR: Color = tailwind::SLATE.c950;
const SELECTED_STYLE_FG: Color = tailwind::BLUE.c300;
const TEXT_COLOR: Color = tailwind::SLATE.c200;
const URL: &str = "http://localhost:8080/";

struct StatefulList<T> {
    state: ListState,
    items: Vec<T>,
    last_selected: Option<usize>,
}

#[derive(Debug)]
enum SelectedLayout {
    Sitzungen,
    Tops,
    Anträge,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Sitzung {
    name: String,
    datum: NaiveDateTime,
    id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Top {
    name: String,
    id: Uuid,
    inhalt: serde_json::Value,
    weight: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Antrag {
    id: Uuid,
    titel: String,
    begründung: String,
    antragstext: String,
}

fn get_sitzungen() -> Vec<Sitzung> {
    let endoint = "api/topmanager/sitzungen/";
    let reqwest = reqwest::blocking::Client::new();
    let response = reqwest.get(URL.to_string() + endoint).send().unwrap();
    let sitzungen: Vec<Sitzung> = response.json().unwrap();
    sitzungen
}

struct App {
    sitzungen: StatefulList<Sitzung>,
    tops_selected_sitzung: StatefulList<Top>,
    anträge_selected_top: StatefulList<Antrag>,
    layout: SelectedLayout,
}

fn main() -> Result<(), Box<dyn Error>> {
    init_error_hooks()?;
    let terminal = init_terminal()?;

    App::new().run(terminal)?;

    restore_terminal()?;

    Ok(())
}

fn init_error_hooks() -> color_eyre::Result<()> {
    let (panic, error) = HookBuilder::default().into_hooks();
    let panic = panic.into_panic_hook();
    let error = error.into_eyre_hook();
    color_eyre::eyre::set_hook(Box::new(move |e| {
        let _ = restore_terminal();
        error(e)
    }))?;
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        panic(info);
    }));
    Ok(())
}

fn init_terminal() -> color_eyre::Result<Terminal<impl Backend>> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal() -> color_eyre::Result<()> {
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

impl<'a> App {
    fn new() -> Self {
        Self {
            sitzungen: StatefulList::with_items(get_sitzungen()),
            tops_selected_sitzung: StatefulList::with_items(vec![]),
            anträge_selected_top: StatefulList::with_items(vec![]),
            layout: SelectedLayout::Sitzungen,
        }
    }

    fn open_sitzung(&mut self) {
        let selected = self.sitzungen.state.selected().unwrap();
        let sitzung = self.sitzungen.items[selected].clone();
        let url = format!("{}api/topmanager/sitzung/{}/tops/", URL, sitzung.id);
        let reqwest = reqwest::blocking::Client::new();
        let response = reqwest.get(url).send().unwrap();
        let tops: Vec<Top> = response.json().unwrap();
        self.tops_selected_sitzung = StatefulList::with_items(tops);
        //open new view with sitzung
        self.layout = SelectedLayout::Tops;
    }

    fn open_top(&mut self) {
        let selected = self.tops_selected_sitzung.state.selected().unwrap();
        let top = self.tops_selected_sitzung.items[selected].clone();
        let url = format!("{}api/topmanager/tops/{}/anträge/", URL, top.id);
        let reqwest = reqwest::blocking::Client::new();
        let response = reqwest.get(url).send().unwrap();
        let antrag: Vec<Antrag> = response.json().unwrap();
        //open new view with top
        self.anträge_selected_top = StatefulList::with_items(antrag);
        self.layout = SelectedLayout::Anträge;
    }

    fn exit_app(&self) {
        std::process::exit(0);
    }
}

impl App {
    fn run(&mut self, mut terminal: Terminal<impl Backend>) -> io::Result<()> {
        loop {
            self.draw(&mut terminal)?;

            if let SelectedLayout::Sitzungen = self.layout {
                self.handle_sitzungen(&mut terminal)?;
            } else if let SelectedLayout::Tops = self.layout {
                self.handle_tops(&mut terminal)?;
            } else {
                self.handle_anträge(&mut terminal)?;
            }
        }
    }

    fn draw(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        terminal.draw(|f| f.render_widget(self, f.size()))?;
        Ok(())
    }

    fn handle_sitzungen(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                use KeyCode::*;
                match key.code {
                    Char('q') | Esc => self.exit_app(),
                    Char('h') | Left => self.sitzungen.unselect(),
                    Char('j') | Down => self.sitzungen.next(),
                    Char('k') | Up => self.sitzungen.previous(),
                    Char('o') => self.open_sitzung(),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn handle_tops(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                use KeyCode::*;
                match key.code {
                    Char('q') | Esc => self.switch_layout(SelectedLayout::Sitzungen),
                    Char('h') | Left => self.tops_selected_sitzung.unselect(),
                    Char('j') | Down => self.tops_selected_sitzung.next(),
                    Char('k') | Up => self.tops_selected_sitzung.previous(),
                    Char('o') => self.open_top(),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn handle_anträge(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                use KeyCode::*;
                match key.code {
                    Char('q') | Esc => self.switch_layout(SelectedLayout::Tops),
                    Char('h') | Left => self.anträge_selected_top.unselect(),
                    Char('j') | Down => self.anträge_selected_top.next(),
                    Char('k') | Up => self.anträge_selected_top.previous(),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn switch_layout(&mut self, layout: SelectedLayout) {
        self.layout = layout;
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let vertical = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(2),
        ]);
        let [header_area, rest_area, footer_area] = vertical.areas(area);

        render_title(header_area, buf);
        if let SelectedLayout::Sitzungen = self.layout {
            self.render_overview_sitzungen(rest_area, buf);
        } else if let SelectedLayout::Tops = self.layout {
            self.render_overview_tops(rest_area, buf);
        } else {
            self.render_anträge(rest_area, buf);
        }
        render_footer(footer_area, buf);
    }
}

impl App {
    fn render_overview(
        &self,
        area: Rect,
        buf: &mut Buffer,
        title: String,
        items: Vec<String>,
        state: &mut ListState,
    ) {
        let outer_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(TODO_HEADER_BG)
            .title(title)
            .title_alignment(Alignment::Center);
        let inner_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(NORMAL_ROW_COLOR);

        let outer_area = area;
        let inner_area = outer_block.inner(outer_area);

        outer_block.render(outer_area, buf);
        let items = List::new(items)
            .block(inner_block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
                    .fg(SELECTED_STYLE_FG),
            )
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(items, inner_area, buf, state);
    }
    fn render_overview_sitzungen(&mut self, area: Rect, buf: &mut Buffer) {
        let outer_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(TODO_HEADER_BG)
            .title("TODO List")
            .title_alignment(Alignment::Center);
        let inner_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(NORMAL_ROW_COLOR);

        let outer_area = area;
        let inner_area = outer_block.inner(outer_area);

        outer_block.render(outer_area, buf);

        let sitzungen = self.sitzungen.items.iter().map(|sitzung| {
            let text = format!("{} {}", sitzung.name, sitzung.datum);
            ListItem::new(text)
        });

        let items = List::new(sitzungen)
            .block(inner_block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
                    .fg(SELECTED_STYLE_FG),
            )
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(items, inner_area, buf, &mut self.sitzungen.state);
    }

    fn render_overview_tops(&mut self, area: Rect, buf: &mut Buffer) {
        // We create two blocks, one is for the header (outer) and the other is for list (inner).
        let outer_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(TODO_HEADER_BG)
            .title("TODO List")
            .title_alignment(Alignment::Center);
        let inner_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(NORMAL_ROW_COLOR);
        // We get the inner area from outer_block. We'll use this area later to render the table.
        let outer_area = area;
        let inner_area = outer_block.inner(outer_area);
        // We can render the header in outer_area.
        outer_block.render(outer_area, buf);
        let tops = self.tops_selected_sitzung.items.iter().map(|top| {
            let text = format!("{} {}", top.weight as i32, top.name);
            ListItem::new(text)
        });
        // Create a List from all list items and highlight the currently selected one
        let items = List::new(tops)
            .block(inner_block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
                    .fg(SELECTED_STYLE_FG),
            )
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);
        // We can now render the item list
        // (look careful we are using StatefulWidget's render.)
        // ratatui::widgets::StatefulWidget::render as stateful_render
        StatefulWidget::render(
            items,
            inner_area,
            buf,
            &mut self.tops_selected_sitzung.state,
        );
    }

    fn render_anträge(&mut self, area: Rect, buf: &mut Buffer) {
        // We create two blocks, one is for the header (outer) and the other is for list (inner).
        let outer_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(TODO_HEADER_BG)
            .title("TODO List")
            .title_alignment(Alignment::Center);
        let inner_block = Block::default()
            .borders(Borders::NONE)
            .fg(TEXT_COLOR)
            .bg(NORMAL_ROW_COLOR);
        // We get the inner area from outer_block. We'll use this area later to render the table.
        let outer_area = area;
        let inner_area = outer_block.inner(outer_area);
        // We can render the header in outer_area.
        outer_block.render(outer_area, buf);
        let anträge = self.anträge_selected_top.items.iter().map(|antrag| {
            let text = format!("{} {}", antrag.titel, antrag.begründung);
            ListItem::new(text)
        });
        // Create a List from all list items and highlight the currently selected one
        let items = List::new(anträge)
            .block(inner_block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
                    .fg(SELECTED_STYLE_FG),
            )
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);
        // We can now render the item list
        // (look careful we are using StatefulWidget's render.)
        // ratatui::widgets::StatefulWidget::render as stateful_render
        StatefulWidget::render(items, inner_area, buf, &mut self.anträge_selected_top.state);
    }
}

fn render_title(area: Rect, buf: &mut Buffer) {
    Paragraph::new("Ratatui List Example")
        .bold()
        .centered()
        .render(area, buf);
}

fn render_footer(area: Rect, buf: &mut Buffer) {
    Paragraph::new("\nUse ↓↑ to move, ← to unselect, → to change status, g/G to go top/bottom.")
        .centered()
        .render(area, buf);
}

impl<T> StatefulList<T> {
    fn with_items(items: Vec<T>) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));

        Self {
            state,
            items,
            last_selected: None,
        }
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if self.items.len() == 1 || self.items.len() - 1 == i {
                    0
                } else {
                    i + 1
                }
            }
            None => self.last_selected.unwrap_or(0),
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => self.last_selected.unwrap_or(0),
        };
        self.state.select(Some(i));
    }

    fn unselect(&mut self) {
        let offset = self.state.offset();
        self.last_selected = self.state.selected();
        self.state.select(None);
        *self.state.offset_mut() = offset;
    }
}
