//! Main application state and logic.
//!
//! Implements the Elm architecture for Bock UI.

#![allow(dead_code)]
#![allow(unused_imports)]

use bockrose::{BockoseSpec, Orchestrator};
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length, Task, Theme};
use std::sync::Arc;

use crate::theme::sizes;

/// Container information for display.
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    /// Container name.
    pub name: String,
    /// Image name.
    pub image: String,
    /// Current status.
    pub status: String,
    /// Mapped ports.
    pub ports: String,
}

/// Active screen in the application.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    /// Main dashboard.
    #[default]
    Dashboard,
    /// Images browser.
    Images,
    /// Networks view.
    Networks,
    /// Volumes view.
    Volumes,
    /// Settings.
    Settings,
}

impl Screen {
    /// Get display name for the screen.
    pub fn label(&self) -> &'static str {
        match self {
            Screen::Dashboard => "Dashboard",
            Screen::Images => "Images",
            Screen::Networks => "Networks",
            Screen::Volumes => "Volumes",
            Screen::Settings => "Settings",
        }
    }

    /// Get Font Awesome icon character for the screen.
    pub fn icon(&self) -> char {
        use crate::icons::icons::*;
        match self {
            Screen::Dashboard => DASHBOARD,
            Screen::Images => IMAGES,
            Screen::Networks => NETWORK,
            Screen::Volumes => DATABASE,
            Screen::Settings => SETTINGS,
        }
    }
}

/// Application state.
/// Application state.
#[derive(Default)]
pub struct BockApp {
    /// Current screen.
    screen: Screen,
    /// Orchestrator instance.
    orchestrator: Option<Arc<Orchestrator>>,
    /// List of containers.
    containers: Vec<ContainerInfo>,
    /// Loading state.
    loading: bool,
    /// Error message.
    error: Option<String>,
}

/// Application messages.
/// Application messages.
#[derive(Debug, Clone)]
pub enum Message {
    /// Navigate to a screen.
    Navigate(Screen),
    /// Refresh container list.
    Refresh,
    /// Orchestrator loaded.
    OrchestratorLoaded(Result<Arc<Orchestrator>, String>),
    /// Containers loaded.
    ContainersLoaded(Result<Vec<ContainerInfo>, String>),
    /// Error occurred.
    Error(String),
    /// Start a container (not implemented yet).
    StartContainer(String),
    /// Stop a container.
    StopContainer(String),
    /// Container action completed.
    ActionCompleted(Result<(), String>),
}

impl BockApp {
    /// Create a new application instance.
    pub fn new() -> (Self, Task<Message>) {
        let app = Self::default();
        // Initialize orchestrator and then refresh
        (
            app,
            Task::perform(init_orchestrator(), Message::OrchestratorLoaded),
        )
    }

    /// Get the window title.
    pub fn title(&self) -> String {
        format!("Bock - {}", self.screen.label())
    }

    /// Update application state based on message.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(screen) => {
                self.screen = screen;
                Task::none()
            }
            Message::Refresh => {
                self.loading = true;
                self.error = None;
                if let Some(orch) = &self.orchestrator {
                    Task::perform(refresh_containers(orch.clone()), Message::ContainersLoaded)
                } else {
                    // Try to init again
                    Task::perform(init_orchestrator(), Message::OrchestratorLoaded)
                }
            }
            Message::OrchestratorLoaded(result) => match result {
                Ok(orch) => {
                    self.orchestrator = Some(orch.clone());
                    self.loading = true;
                    Task::perform(refresh_containers(orch), Message::ContainersLoaded)
                }
                Err(e) => {
                    self.error = Some(e);
                    self.loading = false;
                    Task::none()
                }
            },
            Message::ContainersLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(containers) => {
                        self.containers = containers;
                        self.error = None;
                    }
                    Err(e) => self.error = Some(e),
                }
                Task::none()
            }
            Message::Error(e) => {
                self.error = Some(e);
                self.loading = false;
                Task::none()
            }
            Message::StartContainer(_name) => Task::none(),
            Message::StopContainer(name) => {
                if let Some(orch) = &self.orchestrator {
                    self.loading = true; // Show loading while stopping
                    Task::perform(stop_container(orch.clone(), name), Message::ActionCompleted)
                } else {
                    Task::none()
                }
            }
            Message::ActionCompleted(result) => {
                if let Err(e) = result {
                    self.error = Some(format!("Action failed: {}", e));
                }
                // Always refresh after action
                self.update(Message::Refresh)
            }
        }
    }

    /// Render the application UI.
    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = self.view_sidebar();
        let content = match self.screen {
            Screen::Dashboard => self.view_dashboard(),
            Screen::Images => self.view_placeholder("Images"),
            Screen::Networks => self.view_placeholder("Networks"),
            Screen::Volumes => self.view_placeholder("Volumes"),
            Screen::Settings => self.view_placeholder("Settings"),
        };

        let main_row = row![
            sidebar,
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(sizes::SPACING_LG)
        ];

        container(main_row)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Render the sidebar navigation.
    fn view_sidebar(&self) -> Element<'_, Message> {
        let nav_items = [
            Screen::Dashboard,
            Screen::Images,
            Screen::Networks,
            Screen::Volumes,
            Screen::Settings,
        ];

        let items: Vec<Element<'_, Message>> = nav_items
            .iter()
            .map(|&screen| {
                let is_active = self.screen == screen;

                // Create icon + label row
                let icon_widget = crate::icons::icon(screen.icon()).size(16);
                let label_widget = text(screen.label()).size(14);
                let content = row![icon_widget, label_widget]
                    .spacing(8)
                    .align_y(iced::Alignment::Center);

                button(content)
                    .width(Length::Fill)
                    .padding(sizes::SPACING)
                    .on_press(Message::Navigate(screen))
                    .style(if is_active {
                        button::primary
                    } else {
                        button::secondary
                    })
                    .into()
            })
            .collect();

        let nav = column(items).spacing(sizes::SPACING_SM);

        let header = container(text("🐋 Bock").size(24)).padding(sizes::SPACING_LG);

        let sidebar_content = column![header, nav]
            .spacing(sizes::SPACING)
            .width(Length::Fixed(sizes::SIDEBAR_WIDTH));

        container(sidebar_content)
            .height(Length::Fill)
            .padding(sizes::SPACING)
            .into()
    }

    /// Render the dashboard screen.
    fn view_dashboard(&self) -> Element<'_, Message> {
        let header = row![
            text("Containers").size(24),
            iced::widget::horizontal_space(),
            button(text("↻ Refresh").size(14))
                .padding(sizes::SPACING_SM)
                .on_press(Message::Refresh)
        ]
        .spacing(sizes::SPACING);

        let content: Element<Message> = if self.loading {
            container(text("Loading...").size(16))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        } else if let Some(ref error) = self.error {
            container(text(format!("Error: {}", error)).size(14))
                .center_x(Length::Fill)
                .into()
        } else if self.containers.is_empty() {
            container(
                column![
                    text("No containers running").size(18),
                    text("Start a stack with bockrose to see containers here").size(14)
                ]
                .spacing(sizes::SPACING_SM)
                .align_x(iced::Alignment::Center),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(sizes::SPACING_LG)
            .into()
        } else {
            let cards: Vec<Element<Message>> = self
                .containers
                .iter()
                .map(|c| self.view_container_card(c))
                .collect();

            scrollable(column(cards).spacing(sizes::SPACING))
                .height(Length::Fill)
                .into()
        };

        column![header, content]
            .spacing(sizes::SPACING_LG)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Render a container card.
    fn view_container_card(&self, cinfo: &ContainerInfo) -> Element<'_, Message> {
        let name = cinfo.name.clone();
        let image = cinfo.image.clone();
        let status_str = cinfo.status.clone();

        let status_dot = text("●")
            .size(12)
            .color(crate::theme::status_color(&status_str));

        let info = column![text(name.clone()).size(16), text(image).size(12),].spacing(4);

        let status = row![status_dot, text(status_str).size(12)].spacing(sizes::SPACING_SM);

        let actions = match status_str.to_lowercase().as_str() {
            "running" => row![
                button(text("Stop").size(12))
                    .padding(6)
                    .on_press(Message::StopContainer(name)),
            ],
            "stopped" | "exited" => row![
                button(text("Start").size(12))
                    .padding(6)
                    .on_press(Message::StartContainer(name)),
            ],
            _ => row![],
        }
        .spacing(sizes::SPACING_SM);

        let card_content = row![info, iced::widget::horizontal_space(), status, actions]
            .spacing(sizes::SPACING)
            .align_y(iced::Alignment::Center);

        container(card_content)
            .width(Length::Fill)
            .padding(sizes::CARD_PADDING)
            .into()
    }

    /// Render a placeholder screen.
    fn view_placeholder(&self, name: &str) -> Element<'_, Message> {
        container(text(format!("{} - Coming Soon", name)).size(24))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    /// Get the theme.
    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}

/// Initialize orchestrator by finding config in current directory.
async fn init_orchestrator() -> Result<Arc<Orchestrator>, String> {
    let path = std::env::current_dir().map_err(|e| e.to_string())?;
    // Try to find configuration file
    let config_names = ["bockrose.yaml", "bockrose.yml", "Bockfile"];
    let mut config_path = None;

    // Check current dir
    for name in config_names {
        let p = path.join(name);
        if p.exists() {
            config_path = Some(p);
            break;
        }
    }

    // Check demo file if in examples
    if config_path.is_none() {
        let p = path.join("examples/bockrose-demo.yaml");
        if p.exists() {
            config_path = Some(p);
        }
    }

    let config_path = config_path
        .ok_or_else(|| format!("No bockrose.yaml or Bockfile found in {}", path.display()))?;

    tracing::info!("Loading config from {:?}", config_path);
    let spec = BockoseSpec::from_file(&config_path).map_err(|e| e.to_string())?;
    let orchestrator = Orchestrator::new(spec).map_err(|e| e.to_string())?;

    Ok(Arc::new(orchestrator))
}

/// Refresh containers from orchestrator.
async fn refresh_containers(orchestrator: Arc<Orchestrator>) -> Result<Vec<ContainerInfo>, String> {
    orchestrator
        .refresh_state()
        .await
        .map_err(|e| e.to_string())?;

    let services = orchestrator.list_services();
    let mut infos = Vec::new();

    for service in services {
        let spec = orchestrator.get_service_spec(&service.name);
        let image = spec
            .as_ref()
            .and_then(|s| s.image.clone())
            .unwrap_or_else(|| "<unknown>".to_string());

        let status = format!("{:?}", service.status);

        // Simple port display
        let ports = spec
            .as_ref()
            .map(|s| s.ports.join(", "))
            .unwrap_or_default();

        infos.push(ContainerInfo {
            name: service.name.clone(),
            image,
            status,
            ports,
        });
    }

    // Sort by name
    infos.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(infos)
}

/// Stop a container service.
async fn stop_container(orchestrator: Arc<Orchestrator>, name: String) -> Result<(), String> {
    orchestrator
        .stop_service(&name)
        .await
        .map_err(|e| e.to_string())
}
