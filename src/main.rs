use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, Text};
use iced::widget::{column, container, row, text_input, pick_list, button, slider, text, horizontal_space, scrollable, Column};
use iced::{mouse, Color, Element, Length, Point, Rectangle, Theme, Size, alignment, Subscription, Event, window, Font};
use iced::font::{Weight};
use chrono::{NaiveDate, Utc, Duration, Datelike, Weekday};
use serde::{Serialize, Deserialize};
use std::fs;
use std::time::{Instant, Duration as StdDuration};

pub fn main() -> iced::Result {
    iced::application("Biorhythm Pro Forecast", BiorhythmApp::update, BiorhythmApp::view)
        .theme(|_| Theme::Dark)
        .subscription(BiorhythmApp::subscription)
        .run()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
struct SavedEntry {
    name: String,
    date: NaiveDate,
}

impl std::fmt::Display for SavedEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.date)
    }
}

struct BiorhythmApp {
    name_input: String,
    date_input: String,
    selected_entry: Option<SavedEntry>,
    saved_entries: Vec<SavedEntry>,
    chart_cache: canvas::Cache,
    day_offset: i32,
    rolling_direction: Option<i32>,
    last_tick: Instant,
}

#[derive(Debug, Clone)]
enum Message {
    NameChanged(String),
    DateChanged(String),
    SaveEntry,
    EntrySelected(SavedEntry),
    OffsetChanged(i32),
    ShiftOffset(i32),
    StartRolling(i32),
    FrameTick(Instant),
    ResetOffset,
    EventOccurred(Event),
    MouseWheelScrolled(f32),
    GoToDate(i32),
}

impl BiorhythmApp {
    fn update(&mut self, message: Message) {
        match message {
            Message::NameChanged(n) => self.name_input = n,
            Message::DateChanged(d) => { self.date_input = d; self.chart_cache.clear(); },
            Message::SaveEntry => {
                if let Ok(date) = NaiveDate::parse_from_str(&self.date_input, "%Y-%m-%d") {
                    let entry = SavedEntry { name: self.name_input.clone(), date };
                    if !self.saved_entries.contains(&entry) {
                        self.saved_entries.push(entry);
                        let _ = fs::write("entries.json", serde_json::to_string(&self.saved_entries).unwrap());
                    }
                }
            },
            Message::EntrySelected(entry) => {
                self.date_input = entry.date.format("%Y-%m-%d").to_string();
                self.name_input = entry.name.clone();
                self.selected_entry = Some(entry);
                self.chart_cache.clear();
            },
            Message::OffsetChanged(val) => { self.day_offset = val; self.chart_cache.clear(); },
            Message::ShiftOffset(delta) => {
                self.day_offset = (self.day_offset + delta).clamp(-45830, 36525);
                self.chart_cache.clear();
            },
            Message::StartRolling(dir) => {
                self.rolling_direction = Some(dir);
                self.last_tick = Instant::now();
            },
            Message::FrameTick(now) => {
                if let Some(dir) = self.rolling_direction {
                    if now - self.last_tick >= StdDuration::from_millis(60) {
                        self.day_offset = (self.day_offset + dir).clamp(-45830, 36525);
                        self.chart_cache.clear();
                        self.last_tick = now;
                    }
                }
            },
            Message::ResetOffset => { self.day_offset = 0; self.chart_cache.clear(); },
            Message::MouseWheelScrolled(y) => {
                let delta = if y > 0.0 { -1 } else { 1 };
                self.day_offset = (self.day_offset + delta).clamp(-45830, 36525);
                self.chart_cache.clear();
            },
            Message::GoToDate(offset) => {
                self.day_offset = offset;
                self.chart_cache.clear();
            }
            Message::EventOccurred(event) => {
                match event {
                    Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                        let y = match delta {
                            mouse::ScrollDelta::Lines { y, .. } => y,
                            mouse::ScrollDelta::Pixels { y, .. } => y.signum(),
                        };
                        self.update(Message::MouseWheelScrolled(y));
                    }
                    Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                        self.rolling_direction = None;
                    }
                    _ => {}
                }
            },
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let events = iced::event::listen().map(Message::EventOccurred);
        if self.rolling_direction.is_some() {
            Subscription::batch(vec![events, window::frames().map(Message::FrameTick)])
        } else {
            events
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let today = Utc::now().naive_utc().date();
        let target_date = today + Duration::days(self.day_offset as i64);
        let target_year = target_date.format("%Y").to_string();

        let controls = row![
            text_input("Name", &self.name_input).on_input(Message::NameChanged),
            text_input("YYYY-MM-DD", &self.date_input).on_input(Message::DateChanged).width(102),
            button("Save").on_press(Message::SaveEntry),
            pick_list(&self.saved_entries[..], self.selected_entry.clone(), Message::EntrySelected).placeholder("Select profile..."),
        ].spacing(10);

        let nav_row = row![
            button("<< Week").on_press(Message::ShiftOffset(-7)),
            button("< Day").on_press(Message::StartRolling(-1)),
            slider(-45830..=36525, self.day_offset, Message::OffsetChanged).width(Length::Fill),
            button("Day >").on_press(Message::StartRolling(1)),
            button("Week >>").on_press(Message::ShiftOffset(7)),
            button("Today").on_press(Message::ResetOffset),
        ].spacing(10).align_y(alignment::Vertical::Center);

        let sidebar = self.build_analysis_sidebar();

        // The header row containing "Critical Days" and the Year aligned with the center of the chart
        let chart_header = row![
            // This container effectively mimics the chart's width and padding
            // to place the year in the absolute center of the chart area.
            container(
                text(target_year)
                    .size(20)
                    .font(Font { weight: Weight::Bold, ..Font::DEFAULT })
            )
            .width(Length::FillPortion(5))
            .align_x(alignment::Horizontal::Center),

            // This container matches the sidebar title "Critical Days"
            container(
                text("Critical Days")
                    .size(20)
                    .font(Font { weight: Weight::Bold, ..Font::DEFAULT })
            )
            .width(Length::FillPortion(1))
        ].spacing(20);

        container(column![
            controls,
            nav_row,
            chart_header,
            row![
                column![
                    Canvas::new(self).width(Length::Fill).height(Length::Fill),
                    row![
                        text(format!("Timeline Offset: {} days", self.day_offset)).size(14),
                        horizontal_space(),
                        text("Physical").color(Color::from_rgb8(255, 80, 80)),
                        text("Emotional").color(Color::from_rgb8(80, 255, 80)),
                        text("Intellectual").color(Color::from_rgb8(80, 80, 255)),
                    ].spacing(20)
                ].width(Length::FillPortion(5)).spacing(10),
                sidebar.width(Length::FillPortion(1))
            ].spacing(20).height(Length::Fill)
        ].spacing(15).padding(30)).into()
    }

    fn build_analysis_sidebar(&self) -> Column<'_, Message> {
        let mut analysis = column![
            text("Zero-crossing events").size(12).color(Color::from_rgb(0.5, 0.5, 0.5))
        ].spacing(10);

        if let Ok(birthday) = NaiveDate::parse_from_str(&self.date_input, "%Y-%m-%d") {
            let today = Utc::now().naive_utc().date();
            let mut items = Vec::new();

            for i in (self.day_offset - 5)..(self.day_offset + 25) {
                let date = today + Duration::days(i as i64);
                let days_since = date.signed_duration_since(birthday).num_days() as f64;

                let mut active_crit = Vec::new();
                for (period, name) in [(23.0, "P"), (28.0, "E"), (33.0, "I")] {
                    let val_now = ((2.0 * std::f64::consts::PI * days_since) / period).sin();
                    let val_prev = ((2.0 * std::f64::consts::PI * (days_since - 1.0)) / period).sin();

                    if (val_now >= 0.0 && val_prev < 0.0) || (val_now <= 0.0 && val_prev > 0.0) {
                        active_crit.push(name);
                    }
                }

                if !active_crit.is_empty() {
                    items.push((i, date, active_crit));
                }
            }

            let list = items.into_iter().fold(column![].spacing(8), |col, (off, date, types)| {
                let color = match types.len() {
                    3 => Color::from_rgb(1.0, 0.2, 0.2),
                    2 => Color::from_rgb(1.0, 0.6, 0.0),
                    _ => Color::from_rgb(0.7, 0.7, 0.7),
                };

                col.push(
                    button(
                        row![
                            text(date.format("%b %d").to_string()).size(14).width(60),
                            text(types.join(" + ")).color(color).size(14).font(Font { weight: Weight::Bold, ..Font::DEFAULT }),
                        ].spacing(10)
                    )
                    .width(Length::Fill)
                    .on_press(Message::GoToDate(off))
                    .style(button::secondary)
                )
            });

            analysis = analysis.push(scrollable(list));
        }
        analysis
    }
}

impl<Message> canvas::Program<Message> for BiorhythmApp {
    type State = ();

    fn draw(&self, _st: &Self::State, renderer: &iced::Renderer, _th: &Theme, bounds: Rectangle, _cur: mouse::Cursor) -> Vec<Geometry> {
        let geometry = self.chart_cache.draw(renderer, bounds.size(), |frame: &mut Frame| {
            let pad_l = 60.0;
            let pad_t = 60.0;
            let chart_w = frame.width() - (pad_l * 1.5);
            let chart_h = frame.height() - (pad_t * 2.0);
            let mid_y = pad_t + (chart_h / 2.0);

            if let Ok(birthday) = NaiveDate::parse_from_str(&self.date_input, "%Y-%m-%d") {
                let today = Utc::now().naive_utc().date();
                let target_date = today + Duration::days(self.day_offset as i64);
                let view_start = target_date - Duration::days(15);
                let days_at_start = view_start.signed_duration_since(birthday).num_days() as f64;

                frame.stroke(&Path::line(Point::new(pad_l, pad_t), Point::new(pad_l, pad_t + chart_h)), Stroke::default().with_color(Color::WHITE).with_width(1.0));
                frame.stroke(&Path::line(Point::new(pad_l, mid_y), Point::new(pad_l + chart_w, mid_y)), Stroke::default().with_color(Color::from_rgb(0.4, 0.4, 0.4)));

                for i in 0..=30 {
                    let x = pad_l + (i as f32 / 30.0) * chart_w;
                    let cur_date = view_start + Duration::days(i as i64);
                    let is_target = cur_date == target_date;
                    // Check if the current date in the loop is a Sunday
                    let is_sunday = cur_date.weekday() == Weekday::Sun;

                    let l_col = if is_target { Color::from_rgba(1.0, 1.0, 0.0, 0.8) } else { Color::from_rgba(1.0, 1.0, 1.0, 0.05) };
                    frame.stroke(&Path::line(Point::new(x, pad_t), Point::new(x, pad_t + chart_h)), Stroke::default().with_color(l_col));

                    if is_target || i % 5 == 0 {
                        frame.fill_text(Text {
                            content: cur_date.format("%d/%m").to_string(),
                            position: Point::new(x, pad_t - 15.0),
                            color: if is_target { Color::from_rgb(1.0, 1.0, 0.0) } else { Color::from_rgb(0.6, 0.6, 0.6) },
                            size: 11.0.into(),
                            horizontal_alignment: alignment::Horizontal::Center,
                            ..Default::default()
                        });
                    }


                    let day_color = if is_target {
                        Color::from_rgb(1.0, 1.0, 0.0)
                    } else if is_sunday {
                        Color::from_rgb(1.0, 0.5, 0.5) // Bright red for "Sun"
                    } else {
                        Color::WHITE
                    };

                    frame.fill_text(Text {
                        content: cur_date.format("%a").to_string(),
                        position: Point::new(x, pad_t + chart_h + 15.0),
                        // color: if is_target { Color::from_rgb(1.0, 1.0, 0.0) } else { Color::WHITE },
                        color: day_color,
                        size: 11.0.into(),
                        horizontal_alignment: alignment::Horizontal::Center,
                        ..Default::default()
                    });
                }

                self.draw_bars(frame, days_at_start, pad_l, chart_w, mid_y, chart_h);
                self.draw_plot(frame, days_at_start, pad_l, chart_w, mid_y, chart_h);
            }
        });
        vec![geometry]
    }
}

impl BiorhythmApp {
    fn draw_bars(&self, frame: &mut Frame, start: f64, pad: f32, w: f32, mid_y: f32, h: f32) {
        let spacing = w / 30.0;
        for i in 0..30 {
            let d = i as f64;
            let p = ((2.0 * std::f64::consts::PI * (start + d)) / 23.0).sin();
            let e = ((2.0 * std::f64::consts::PI * (start + d)) / 28.0).sin();
            let intel = ((2.0 * std::f64::consts::PI * (start + d)) / 33.0).sin();
            let avg = (p + e + intel) / 3.0;

            let bar_h = (avg as f32 * (h / 2.0)).abs();
            let color = if avg >= 0.0 { Color::from_rgba(0.2, 1.0, 0.5, 0.2) } else { Color::from_rgba(1.0, 0.3, 0.3, 0.2) };

            frame.fill_rectangle(
                Point::new(pad + (i as f32 * spacing) + (spacing * 0.1), if avg >= 0.0 { mid_y - bar_h } else { mid_y }),
                Size::new(spacing * 0.8, bar_h),
                color
            );
        }
    }

    fn draw_plot(&self, frame: &mut Frame, start: f64, pad: f32, w: f32, mid_y: f32, h: f32) {
        let cycles = [(23.0, Color::from_rgb8(255, 80, 80)), (28.0, Color::from_rgb8(80, 255, 80)), (33.0, Color::from_rgb8(80, 80, 255))];
        for (period, col) in cycles {
            let mut path = canvas::path::Builder::new();
            for i in 0..=300 {
                let d_off = (i as f64 / 300.0) * 30.0;
                let val = ((2.0 * std::f64::consts::PI * (start + d_off)) / period).sin();
                let x = pad + (i as f32 / 300.0) * w;
                let y = mid_y - (val as f32 * (h / 2.0));

                if val.abs() < 0.015 {
                    frame.fill_rectangle(Point::new(x - 3.0, mid_y - 3.0), Size::new(6.0, 6.0), Color::WHITE);
                }

                if i == 0 { path.move_to(Point::new(x, y)); } else { path.line_to(Point::new(x, y)); }
            }
            frame.stroke(&path.build(), Stroke::default().with_color(col).with_width(2.5));
        }
    }
}

impl Default for BiorhythmApp {
    fn default() -> Self {
        let saved_entries = fs::read_to_string("entries.json").ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Self {
            name_input: String::new(),
            date_input: Utc::now().naive_utc().date().format("%Y-%m-%d").to_string(),
            selected_entry: None,
            saved_entries,
            chart_cache: canvas::Cache::default(),
            day_offset: 0,
            rolling_direction: None,
            last_tick: Instant::now(),
        }
    }
}

