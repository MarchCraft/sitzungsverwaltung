use chrono::NaiveDateTime;
use color_eyre::config::HookBuilder;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{
        self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
    ExecutableCommand,
};
use oauth2::http::header;
use ratatui::{prelude::*, style::palette::tailwind, widgets::*};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    io::{self, stdout},
    time::Duration,
    vec,
};
use tui_textarea::TextArea;
use uuid::Uuid;

const TODO_HEADER_BG: Color = tailwind::BLUE.c950;
const NORMAL_ROW_COLOR: Color = tailwind::SLATE.c950;
const SELECTED_STYLE_FG: Color = tailwind::BLUE.c300;
const TEXT_COLOR: Color = tailwind::SLATE.c200;
const URL: &str = "https://new.hhu-fscs.de/";

mod keycloak;

#[derive(Debug, Clone)]
struct StatefulList<T> {
    state: ListState,
    items: Vec<T>,
    last_selected: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum SelectedLayout {
    Sitzungen,
    Tops,
    Anträge,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct Sitzung {
    name: String,
    datum: NaiveDateTime,
    id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct Top {
    name: String,
    id: Uuid,
    inhalt: serde_json::Value,
    weight: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct Antrag {
    id: Uuid,
    titel: String,
    begründung: String,
    antragstext: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Param {
    titel: String,
    text: String,
}

fn get_sitzungen() -> Vec<Sitzung> {
    let endoint = "api/topmanager/sitzungen/";
    let reqwest = reqwest::blocking::Client::new();
    let response = reqwest.get(URL.to_string() + endoint).send().unwrap();
    let sitzungen: Vec<Sitzung> = response.json().unwrap();
    sitzungen
}

fn get_tops(sitzung: Sitzung) -> Vec<Top> {
    let url = format!("{}api/topmanager/sitzung/{}/tops/", URL, sitzung.id);
    let reqwest = reqwest::blocking::Client::new();
    let response = reqwest.get(url).send().unwrap();
    let tops: Vec<Top> = response.json().unwrap();
    tops
}

fn get_anträge(top: Top) -> Vec<Antrag> {
    let url = format!("{}api/topmanager/tops/{}/anträge/", URL, top.id);
    let reqwest = reqwest::blocking::Client::new();
    let response = reqwest.get(url).send().unwrap();
    let anträge: Vec<Antrag> = response.json().unwrap();
    anträge
}

struct App<'a> {
    sitzungen: StatefulList<Sitzung>,
    tops_selected_sitzung: StatefulList<Top>,
    anträge_selected_top: StatefulList<Antrag>,
    layout: SelectedLayout,
    currently_editing: Option<SelectedLayout>,
    edit_buffer: StatefulList<Param>,
    currently_creating: Option<SelectedLayout>,
    edit_param_pop: Option<Param>,
    current_text_area: TextArea<'a>,
    sitzung: Sitzung,
    top: Top,
    token: String,
    antrag: Antrag,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_error_hooks()?;
    let terminal = init_terminal()?;

    App::new().await.run(terminal)?;

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

impl<'a> App<'_> {
    async fn new() -> Self {
        Self {
            sitzungen: StatefulList::with_items(get_sitzungen()),
            tops_selected_sitzung: StatefulList::with_items(vec![]),
            anträge_selected_top: StatefulList::with_items(vec![]),
            layout: SelectedLayout::Sitzungen,
            currently_editing: None,
            currently_creating: None,
            edit_buffer: StatefulList::with_items(vec![]),
            edit_param_pop: None,
            current_text_area: TextArea::default(),
            sitzung: Sitzung::default(),
            top: Top::default(),
            token: keycloak::get_token().await.unwrap(),
            antrag: Antrag::default(),
        }
    }

    fn get_sitzungen(&mut self) {
        let endoint = "api/topmanager/sitzungen/";
        let reqwest = reqwest::blocking::Client::new();
        let response = reqwest.get(URL.to_string() + endoint).send().unwrap();
        let sitzungen: Vec<Sitzung> = response.json().unwrap();
        self.sitzungen = StatefulList::with_items(sitzungen);
    }

    fn open_sitzung(&mut self) {
        let selected = self.sitzungen.state.selected().unwrap();
        let sitzung = self.sitzungen.items[selected].clone();
        let url = format!("{}api/topmanager/sitzung/{}/tops/", URL, sitzung.id);
        let reqwest = reqwest::blocking::Client::new();
        let response = reqwest.get(url).send().unwrap();
        let tops: Vec<Top> = response.json().unwrap();
        self.sitzung = sitzung;
        self.tops_selected_sitzung = StatefulList::with_items(tops);
        //open new view with sitzung
        self.layout = SelectedLayout::Tops;
    }

    fn create_sitzung(&mut self) {
        self.edit_buffer.items.push(Param {
            titel: "Datum".to_string(),
            text: "".to_string(),
        });
        self.edit_buffer.items.push(Param {
            titel: "Name".to_string(),
            text: "".to_string(),
        });
        self.currently_creating = Some(SelectedLayout::Sitzungen);
    }

    fn delete_sitzung(&mut self) {
        let token = self.token.clone();
        let cookie = format!("access_token={}", token);
        let selected = self.sitzungen.state.selected().unwrap();
        let sitzung = self.sitzungen.items[selected].clone();
        let url = format!("{}api/topmanager/sitzung/", URL);
        let reqwest = reqwest::blocking::Client::new();
        let json = serde_json::json!({ "id": sitzung.id });
        let response = reqwest
            .delete(url)
            .header("Cookie", cookie)
            .json(&json)
            .send()
            .unwrap();
        self.get_sitzungen();
    }

    fn open_top(&mut self) {
        let selected = self.tops_selected_sitzung.state.selected().unwrap();
        let top = self.tops_selected_sitzung.items[selected].clone();
        let url = format!("{}api/topmanager/tops/{}/anträge/", URL, top.id);
        let reqwest = reqwest::blocking::Client::new();
        let response = reqwest.get(url).send().unwrap();
        let antrag: Vec<Antrag> = response.json().unwrap();
        self.top = top;
        //open new view with top
        self.anträge_selected_top = StatefulList::with_items(antrag);
        self.layout = SelectedLayout::Anträge;
    }

    fn create_top(&mut self) {
        self.edit_buffer.items.push(Param {
            titel: "Titel".to_string(),
            text: "".to_string(),
        });
        self.edit_buffer.items.push(Param {
            titel: "Inhalt".to_string(),
            text: "".to_string(),
        });
        self.currently_creating = Some(SelectedLayout::Tops);
    }

    fn delete_top(&mut self) {
        let token = self.token.clone();
        let cookie = format!("access_token={}", token);
        let selected = self.tops_selected_sitzung.state.selected().unwrap();
        let top = self.tops_selected_sitzung.items[selected].clone();
        let url = format!("{}api/topmanager/top/", URL);
        let reqwest = reqwest::blocking::Client::new();
        let json = serde_json::json!({ "id": top.id });
        let response = reqwest
            .delete(url)
            .header("Cookie", cookie)
            .json(&json)
            .send()
            .unwrap();
        self.tops_selected_sitzung = StatefulList::with_items(get_tops(self.sitzung.clone()));
    }

    fn edit_antag(&mut self) {
        let selected = self.anträge_selected_top.state.selected().unwrap();
        let antrag = self.anträge_selected_top.items[selected].clone();
        let url = format!("{}api/topmanager/antrag/{}/", URL, antrag.id);
        let reqwest = reqwest::blocking::Client::new();
        let response = reqwest.get(url).send().unwrap();
        let antrag: Antrag = response.json().unwrap();
        self.antrag = antrag.clone();
        self.edit_buffer.items.push(Param {
            titel: "Titel".to_string(),
            text: antrag.titel,
        });
        self.edit_buffer.items.push(Param {
            titel: "Begründung".to_string(),
            text: antrag.begründung,
        });
        self.edit_buffer.items.push(Param {
            titel: "Antragstext".to_string(),
            text: antrag.antragstext,
        });

        self.currently_editing = Some(SelectedLayout::Anträge);
    }

    fn edit_sitzung(&mut self) {
        let selected = self.sitzungen.state.selected().unwrap();
        let sitzung = self.sitzungen.items[selected].clone();
        let url = format!("{}api/topmanager/sitzung/{}/", URL, sitzung.id);
        let reqwest = reqwest::blocking::Client::new();
        let response = reqwest.get(url).send().unwrap();
        let sitzung: Sitzung = response.json().unwrap();
        self.edit_buffer.items.push(Param {
            titel: "Datum".to_string(),
            text: sitzung.datum.to_string(),
        });
        self.edit_buffer.items.push(Param {
            titel: "Name".to_string(),
            text: sitzung.name,
        });
        self.currently_editing = Some(SelectedLayout::Sitzungen);
    }

    fn edit_top(&mut self) {
        let selected = self.tops_selected_sitzung.state.selected().unwrap();
        let top = self.tops_selected_sitzung.items[selected].clone();
        let url = format!("{}api/topmanager/tops/{}/", URL, top.id);
        let reqwest = reqwest::blocking::Client::new();
        let response = reqwest.get(url).send().unwrap();
        let top: Top = response.json().unwrap();
        self.edit_buffer.items.push(Param {
            titel: "Titel".to_string(),
            text: top.name,
        });
        self.edit_buffer.items.push(Param {
            titel: "Inhalt".to_string(),
            text: top.inhalt.to_string(),
        });
        self.currently_editing = Some(SelectedLayout::Tops);
    }

    fn create_antrag(&mut self) {
        self.edit_buffer.items.push(Param {
            titel: "Titel".to_string(),
            text: "".to_string(),
        });
        self.edit_buffer.items.push(Param {
            titel: "Begründung".to_string(),
            text: "".to_string(),
        });
        self.edit_buffer.items.push(Param {
            titel: "Antragstext".to_string(),
            text: "".to_string(),
        });
        self.edit_buffer.items.push(Param {
            titel: "Antragssteller".to_string(),
            text: "".to_string(),
        });
        self.currently_creating = Some(SelectedLayout::Anträge);
    }

    fn delete_antrag(&mut self) {
        let token = self.token.clone();
        let cookie = format!("access_token={}", token);
        let selected = self.anträge_selected_top.state.selected().unwrap();
        let antrag = self.anträge_selected_top.items[selected].clone();
        let url = format!("{}api/topmanager/antrag/{}/", URL, antrag.id);
        let reqwest = reqwest::blocking::Client::new();
        let response = reqwest.delete(url).header("Cookie", cookie).send().unwrap();
        self.anträge_selected_top = StatefulList::with_items(get_anträge(self.top.clone()));
    }

    fn edit_value(&mut self) {
        let selected = self.edit_buffer.state.selected().unwrap();
        let param = self.edit_buffer.items[selected].clone();
        self.edit_param_pop = Some(param);
        let param = self.edit_param_pop.as_ref().unwrap();
        let text = &param.text;
        self.current_text_area = TextArea::default();
        self.current_text_area.insert_str(text);
    }

    fn exit_app(&self) {
        std::process::exit(0);
    }

    fn patch(&mut self) {
        let token = self.token.clone();
        let cookie = format!("access_token={}", token);

        if let Some(SelectedLayout::Sitzungen) = self.currently_editing {
            let sitzung = &self.sitzung;
            let url = format!("{}api/topmanager/sitzung/", URL);
            let reqwest = reqwest::blocking::Client::new();
            let mut data = serde_json::json!({});
            data["id"] = serde_json::Value::String(sitzung.id.to_string());
            for param in &self.edit_buffer.items {
                data[param.titel.clone().to_lowercase()] =
                    serde_json::Value::String((param.text).to_string());
            }
            let response = reqwest
                .patch(url)
                .header("Cookie", cookie)
                .json(&data)
                .send()
                .unwrap();
        } else if let Some(SelectedLayout::Tops) = self.currently_editing {
            let sitzung = &self.sitzung;
            let selected = self.tops_selected_sitzung.state.selected().unwrap();
            let top = self.tops_selected_sitzung.items[selected].clone();
            let url = format!("{}api/topmanager/top/", URL);
            let reqwest = reqwest::blocking::Client::new();
            let mut data = serde_json::json!({});
            data["id"] = serde_json::Value::String(top.id.to_string());
            data["sitzung_id"] = serde_json::Value::String(sitzung.id.to_string());
            for param in &self.edit_buffer.items {
                data[param.titel.clone().to_lowercase()] =
                    serde_json::Value::String((param.text).to_string());
            }
            let response = reqwest
                .patch(url)
                .header("Cookie", cookie)
                .json(&data)
                .send()
                .unwrap();
        } else if let Some(SelectedLayout::Anträge) = self.currently_editing {
            let antrag = &self.antrag;
            let url = format!("{}api/topmanager/antrag/", URL);
            let reqwest = reqwest::blocking::Client::new();
            let mut data = serde_json::json!({});
            data["id"] = serde_json::Value::String(antrag.id.to_string());
            for param in &self.edit_buffer.items {
                data[param.titel.clone().to_lowercase()] =
                    serde_json::Value::String((param.text).to_string());
            }
            let response = reqwest
                .patch(url)
                .header("Cookie", cookie)
                .json(&data)
                .send()
                .unwrap();
        }
    }

    fn put(&mut self) {
        let token = self.token.clone();
        let cookie = format!("access_token={}", token);

        if let Some(SelectedLayout::Sitzungen) = self.currently_creating {
            let url = format!("{}api/topmanager/sitzung/", URL);
            let reqwest = reqwest::blocking::Client::new();
            let mut data = serde_json::json!({});
            for param in &self.edit_buffer.items {
                data[param.titel.clone().to_lowercase()] =
                    serde_json::Value::String((param.text).to_string());
            }
            let response = reqwest
                .put(url)
                .header("Cookie", cookie)
                .json(&data)
                .send()
                .unwrap();
        } else if let Some(SelectedLayout::Tops) = self.currently_creating {
            let url = format!("{}api/topmanager/sitzung/{}/top/", URL, self.sitzung.id);
            let reqwest = reqwest::blocking::Client::new();
            let mut data = serde_json::json!({});
            for param in &self.edit_buffer.items {
                data[param.titel.clone().to_lowercase()] =
                    serde_json::Value::String((param.text).to_string());
            }
            let response = reqwest
                .put(url)
                .header("Cookie", cookie)
                .json(&data)
                .send()
                .unwrap();
        } else if let Some(SelectedLayout::Anträge) = self.currently_creating {
            let url = format!("{}api/topmanager/top/{}/antrag/", URL, self.top.id);
            let reqwest = reqwest::blocking::Client::new();
            let mut data = serde_json::json!({});
            for param in &self.edit_buffer.items {
                data[param.titel.clone().to_lowercase()] =
                    serde_json::Value::String((param.text).to_string());
            }

            let response = reqwest
                .put(url)
                .header("Cookie", cookie)
                .json(&data)
                .send()
                .unwrap();
        }
    }

    fn update(&mut self) {
        let value = self.current_text_area.lines().concat();
        self.edit_buffer.items[self.edit_buffer.state.selected().unwrap()].text = value;
    }
}

impl App<'_> {
    fn run(&mut self, mut terminal: Terminal<impl Backend>) -> io::Result<()> {
        loop {
            self.draw(&mut terminal)?;
            if self.edit_param_pop.is_some() {
                self.handle_text_area()?;
            } else {
                if self.currently_editing.is_some() {
                    if let Some(SelectedLayout::Sitzungen) = self.currently_editing {
                        //edit sitzung
                        self.handle_edit(&mut terminal)?;
                    } else if let Some(SelectedLayout::Tops) = self.currently_editing {
                        //edit top
                        self.handle_edit(&mut terminal)?;
                    } else if let Some(SelectedLayout::Anträge) = self.currently_editing {
                        //edit antrag
                        self.handle_edit(&mut terminal)?;
                    }
                } else if self.currently_creating.is_some() {
                    if let Some(SelectedLayout::Sitzungen) = self.currently_creating {
                        //edit sitzung
                        self.handle_edit(&mut terminal)?;
                    } else if let Some(SelectedLayout::Tops) = self.currently_creating {
                        //edit top
                        self.handle_edit(&mut terminal)?;
                    } else if let Some(SelectedLayout::Anträge) = self.currently_creating {
                        //edit antrag
                        self.handle_edit(&mut terminal)?;
                    }
                } else if let SelectedLayout::Sitzungen = self.layout {
                    self.handle_sitzungen(&mut terminal)?;
                } else if let SelectedLayout::Tops = self.layout {
                    self.handle_tops(&mut terminal)?;
                } else {
                    self.handle_anträge(&mut terminal)?;
                }
            }
        }
    }

    fn draw(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        terminal.draw(|f| f.render_widget(self, f.size()))?;
        Ok(())
    }

    fn handle_edit(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                use KeyCode::*;
                match key.code {
                    Char('q') | Esc => self.exit_edit(),
                    Char('h') | Left => self.edit_buffer.unselect(),
                    Char('j') | Down => self.edit_buffer.next(),
                    Char('k') | Up => self.edit_buffer.previous(),
                    Char('e') => self.edit_value(),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn handle_text_area(&mut self) -> io::Result<()> {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Esc => {
                        self.update();
                        self.edit_param_pop = None;
                    }
                    _ => {
                        self.current_text_area.input(key);
                    }
                }
            }
        }
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
                    Char('e') => self.edit_sitzung(),
                    Char('p') => self.create_sitzung(),
                    Char('d') => self.delete_sitzung(),
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
                    Char('e') => self.edit_top(),
                    Char('p') => self.create_top(),
                    Char('d') => self.delete_top(),
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
                    Char('e') => self.edit_antag(),
                    Char('p') => self.create_antrag(),
                    Char('d') => self.delete_antrag(),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn switch_layout(&mut self, layout: SelectedLayout) {
        self.layout = layout.clone();
    }

    fn exit_edit(&mut self) {
        if let Some(editing) = &self.currently_editing {
            self.patch();
            self.currently_editing = None;
        }
        if let Some(creating) = &self.currently_creating {
            self.put();
            self.currently_creating = None;
        }
        self.sitzungen = StatefulList::with_items(get_sitzungen());
        self.tops_selected_sitzung = StatefulList::with_items(get_tops(self.sitzung.clone()));
        self.anträge_selected_top = StatefulList::with_items(get_anträge(self.top.clone()));
        self.edit_buffer = StatefulList::with_items(vec![]);
    }
}

impl Widget for &mut App<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let vertical = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(2),
        ]);
        let [header_area, rest_area, footer_area] = vertical.areas(area);

        render_title(header_area, buf);
        if let Some(editing) = &self.edit_param_pop {
            self.render_edit_param(rest_area, buf);
        } else if let Some(editing) = &self.currently_editing {
            self.render_edit(rest_area, buf);
        } else if let Some(creating) = &self.currently_creating {
            self.render_edit(rest_area, buf);
        } else {
            self.render_overview(rest_area, buf);
        }
        render_footer(footer_area, buf);
    }
}

impl App<'_> {
    fn render_overview(&mut self, area: Rect, buf: &mut Buffer) {
        let title = match self.layout {
            SelectedLayout::Sitzungen => "Sitzungen",
            SelectedLayout::Tops => "Tops",
            SelectedLayout::Anträge => "Anträge",
        };
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

        let mut listelement: Vec<String> = vec![];
        if let SelectedLayout::Sitzungen = self.layout {
            listelement = self
                .sitzungen
                .items
                .iter()
                .map(|s| s.name.clone())
                .collect();
        } else if let SelectedLayout::Tops = self.layout {
            listelement = self
                .tops_selected_sitzung
                .items
                .iter()
                .map(|t| t.name.clone())
                .collect();
        } else {
            listelement = self
                .anträge_selected_top
                .items
                .iter()
                .map(|a| a.titel.clone())
                .collect();
        }
        let items = List::new(listelement)
            .block(inner_block)
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::REVERSED)
                    .fg(SELECTED_STYLE_FG),
            )
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        if let SelectedLayout::Sitzungen = self.layout {
            StatefulWidget::render(items, inner_area, buf, &mut self.sitzungen.state);
        } else if let SelectedLayout::Tops = self.layout {
            StatefulWidget::render(
                items,
                inner_area,
                buf,
                &mut self.tops_selected_sitzung.state,
            );
        } else {
            StatefulWidget::render(items, inner_area, buf, &mut self.anträge_selected_top.state);
        }
    }
    fn render_edit(&mut self, area: Rect, buf: &mut Buffer) {
        if let Some(editing) = &self.currently_editing {
            let title = match editing {
                SelectedLayout::Sitzungen => "Edit Sitzung",
                SelectedLayout::Tops => "Edit Top",
                SelectedLayout::Anträge => "Edit Antrag",
            };
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

            let mut listelement: Vec<String> = vec![];
            listelement = self
                .edit_buffer
                .items
                .iter()
                .map(|p| format!("{}: {}", p.titel, p.text))
                .collect();

            let items = List::new(listelement)
                .block(inner_block)
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(Modifier::REVERSED)
                        .fg(SELECTED_STYLE_FG),
                )
                .highlight_symbol(">")
                .highlight_spacing(HighlightSpacing::Always);
            StatefulWidget::render(items, inner_area, buf, &mut self.edit_buffer.state);
        }

        if let Some(editing) = &self.currently_creating {
            let title = match editing {
                SelectedLayout::Sitzungen => "Create Sitzung",
                SelectedLayout::Tops => "Create Top",
                SelectedLayout::Anträge => "Create Antrag",
            };
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

            let mut listelement: Vec<String> = vec![];
            listelement = self
                .edit_buffer
                .items
                .iter()
                .map(|p| format!("{}: {}", p.titel, p.text))
                .collect();

            let items = List::new(listelement)
                .block(inner_block)
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(Modifier::REVERSED)
                        .fg(SELECTED_STYLE_FG),
                )
                .highlight_symbol(">")
                .highlight_spacing(HighlightSpacing::Always);
            StatefulWidget::render(items, inner_area, buf, &mut self.edit_buffer.state);
        }
    }

    fn render_edit_param(&mut self, area: Rect, buf: &mut Buffer) {
        let popup_layout = centered_rect(50, 50, area);
        let popup = Block::default()
            .title("Edit Value")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(TEXT_COLOR))
            .title_style(Style::default().fg(TEXT_COLOR))
            .style(Style::default().bg(NORMAL_ROW_COLOR).fg(TEXT_COLOR));
        let param = self.edit_param_pop.as_ref().unwrap();
        let tile = &param.titel;
        let text = &param.text;
        self.current_text_area
            .set_block(Block::default().title(tile.clone()));
        self.current_text_area
            .widget()
            .render(popup_layout.inner(&Margin::new(2, 2)), buf);
        popup.render(popup_layout, buf);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_title(area: Rect, buf: &mut Buffer) {
    Paragraph::new("Ratatui List Example")
        .bold()
        .centered()
        .render(area, buf);
}

fn render_footer(area: Rect, buf: &mut Buffer) {
    Paragraph::new("\nUse ↓↑ to move, o to open, p to create a new entry, e to edit and q/ESC to exit, d to delete")
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
