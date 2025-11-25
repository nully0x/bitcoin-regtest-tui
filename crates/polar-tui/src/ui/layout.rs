#! Main layout rendering for the TUI.

use polar_core::{BitcoinNodeInfo, LndNodeInfo, NodeInfo};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::app::{ActivePanel, App, UiMode};

/// Render the entire UI.
pub fn render(frame: &mut Frame, app: &App) {
    match app.ui_mode {
        UiMode::CreateNetwork => render_create_network(frame, app),
        UiMode::Main => render_main(frame, app),
        UiMode::NodeDetails => render_node_details(frame, app),
        UiMode::MineBlocks => render_mine_blocks(frame, app),
        UiMode::FundWallet => render_fund_wallet(frame, app),
        UiMode::OpenChannel => render_open_channel(frame, app),
        UiMode::CloseChannel => render_close_channel(frame, app),
        UiMode::SendPayment => render_send_payment(frame, app),
    }
}

/// Render the main application view.
fn render_main(frame: &mut Frame, app: &App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(frame.area());

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(35),
            Constraint::Percentage(40),
        ])
        .split(main_chunks[0]);

    render_networks_panel(frame, app, chunks[0]);
    render_nodes_panel(frame, app, chunks[1]);
    render_logs_panel(frame, app, chunks[2]);
    render_status_bar(frame, app, main_chunks[1]);
}

/// Render the create network dialog.
fn render_create_network(frame: &mut Frame, app: &App) {
    use polar_nodes::{BITCOIN_VERSIONS, LND_VERSIONS};

    // Center the dialog
    let area = centered_rect(85, 70, frame.area());

    // Clear the background
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        frame.area(),
    );

    // Create the dialog content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(3), // Network name
            Constraint::Length(3), // Alias
            Constraint::Length(3), // LND count
            Constraint::Length(3), // LND version
            Constraint::Length(3), // Bitcoin version
            Constraint::Min(1),    // Help text
        ])
        .split(area);

    // Dialog box
    let block = Block::default()
        .title(" Create Network ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    frame.render_widget(block, area);

    // Title
    let title = Paragraph::new("Configure your Lightning Network").style(
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(title, chunks[0]);

    // Helper function for field style
    let field_style = |field_idx: usize| {
        if app.create_form_field == field_idx {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    };

    // Network name field (field 0)
    let name_text = if app.create_network_name.is_empty() {
        Line::from(vec![
            Span::styled("Network Name: ", field_style(0)),
            Span::styled("_", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled("Network Name: ", field_style(0)),
            Span::styled(&app.create_network_name, field_style(0)),
            Span::styled("_", Style::default().fg(Color::Yellow)),
        ])
    };
    frame.render_widget(Paragraph::new(name_text), chunks[1]);

    // Alias field (field 1)
    let alias_text = if app.create_node_alias.is_empty() {
        Line::from(vec![
            Span::styled("Node Alias Prefix: ", field_style(1)),
            Span::styled(
                "(defaults to network name)",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled("Node Alias Prefix: ", field_style(1)),
            Span::styled(&app.create_node_alias, field_style(1)),
            Span::styled("_", Style::default().fg(Color::Yellow)),
        ])
    };
    frame.render_widget(Paragraph::new(alias_text), chunks[2]);

    // LND count field (field 2)
    let count_text = Line::from(vec![
        Span::styled("LND Nodes: ", field_style(2)),
        Span::styled("< ", Style::default().fg(Color::DarkGray)),
        Span::styled(app.create_lnd_count.to_string(), field_style(2)),
        Span::styled(" >", Style::default().fg(Color::DarkGray)),
        Span::styled("  (use ←/→)", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(count_text), chunks[3]);

    // LND version field (field 3)
    let lnd_ver = LND_VERSIONS
        .get(app.create_lnd_version_idx)
        .unwrap_or(&"unknown");
    let lnd_ver_short = lnd_ver.split(':').last().unwrap_or(lnd_ver);
    let lnd_version_text = Line::from(vec![
        Span::styled("LND Version: ", field_style(3)),
        Span::styled("< ", Style::default().fg(Color::DarkGray)),
        Span::styled(lnd_ver_short, field_style(3)),
        Span::styled(" >", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(
                "  ({}/{})",
                app.create_lnd_version_idx + 1,
                LND_VERSIONS.len()
            ),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(lnd_version_text), chunks[4]);

    // Bitcoin version field (field 4)
    let btc_ver = BITCOIN_VERSIONS
        .get(app.create_btc_version_idx)
        .unwrap_or(&"unknown");
    let btc_ver_short = btc_ver.split(':').last().unwrap_or(btc_ver);
    let btc_version_text = Line::from(vec![
        Span::styled("Bitcoin Version: ", field_style(4)),
        Span::styled("< ", Style::default().fg(Color::DarkGray)),
        Span::styled(btc_ver_short, field_style(4)),
        Span::styled(" >", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!(
                "  ({}/{})",
                app.create_btc_version_idx + 1,
                BITCOIN_VERSIONS.len()
            ),
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(btc_version_text), chunks[5]);

    // Help text - all shortcuts on the same line
    let help = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Tab/↑↓", Style::default().fg(Color::Cyan)),
            Span::raw(": Switch field  |  "),
            Span::styled("←/→", Style::default().fg(Color::Cyan)),
            Span::raw(": Adjust value  |  "),
            Span::styled("Enter", Style::default().fg(Color::Green)),
            Span::raw(": Create  |  "),
            Span::styled("Esc", Style::default().fg(Color::Red)),
            Span::raw(": Quit  |  "),
            Span::styled("q", Style::default().fg(Color::Red)),
            Span::raw(": Quit"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "All LND nodes will connect to 1 Bitcoin Core node in regtest mode",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )),
    ];
    frame.render_widget(Paragraph::new(help).wrap(Wrap { trim: false }), chunks[6]);
}

/// Render the networks panel (left).
fn render_networks_panel(frame: &mut Frame, app: &App, area: Rect) {
    let style = panel_style(app.active_panel == ActivePanel::Networks);

    let items: Vec<ListItem> = app
        .networks
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let content = if Some(i) == app.selected_network {
                Line::from(vec![
                    Span::raw("> "),
                    Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
                ])
            } else {
                Line::from(format!("  {name}"))
            };
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Networks ")
            .borders(Borders::ALL)
            .border_style(style),
    );

    frame.render_widget(list, area);
}

/// Render the nodes panel (center).
fn render_nodes_panel(frame: &mut Frame, app: &App, area: Rect) {
    let style = panel_style(app.active_panel == ActivePanel::Nodes);

    let items: Vec<ListItem> = app
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            let content = if Some(i) == app.selected_node {
                Line::from(vec![
                    Span::raw("> "),
                    Span::styled(node, Style::default().add_modifier(Modifier::BOLD)),
                ])
            } else {
                Line::from(format!("  {node}"))
            };
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Nodes ")
            .borders(Borders::ALL)
            .border_style(style),
    );

    frame.render_widget(list, area);
}

/// Render the logs panel (right).
fn render_logs_panel(frame: &mut Frame, app: &App, area: Rect) {
    let style = panel_style(app.active_panel == ActivePanel::Logs);

    let text: Vec<Line> = app.logs.iter().map(|l| Line::from(l.as_str())).collect();

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Logs ")
                .borders(Borders::ALL)
                .border_style(style),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

/// Render the status bar (bottom).
fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let help_text = vec![Line::from(vec![
        Span::raw("Tab: Switch | ↑↓/k/j: Navigate | "),
        Span::styled("n", Style::default().fg(Color::Cyan)),
        Span::raw(": New | "),
        Span::styled("s", Style::default().fg(Color::Green)),
        Span::raw(": Start | "),
        Span::styled("x", Style::default().fg(Color::Red)),
        Span::raw(": Stop | "),
        Span::styled("d", Style::default().fg(Color::Red)),
        Span::raw(": Delete | "),
        Span::styled("i", Style::default().fg(Color::Magenta)),
        Span::raw(": Info | "),
        Span::styled("m", Style::default().fg(Color::Yellow)),
        Span::raw(": Mine | "),
        Span::styled("f", Style::default().fg(Color::Yellow)),
        Span::raw(": Fund | "),
        Span::styled("c", Style::default().fg(Color::Yellow)),
        Span::raw(": Open | "),
        Span::styled("l", Style::default().fg(Color::Red)),
        Span::raw(": Close | "),
        Span::styled("p", Style::default().fg(Color::Yellow)),
        Span::raw(": Payment | "),
        Span::styled("g", Style::default().fg(Color::Cyan)),
        Span::raw(": Graph | "),
        Span::styled("y", Style::default().fg(Color::Cyan)),
        Span::raw(": Chain | "),
        Span::raw("q: Quit"),
    ])];

    let mut status_lines = help_text;

    if let Some(ref msg) = app.status_message {
        // Determine if this is an error message
        let is_error = msg.contains("Failed") || msg.contains("Error") || msg.contains("error");

        status_lines.push(Line::from(vec![
            Span::styled(
                if is_error { "⚠ Error: " } else { "Status: " },
                Style::default()
                    .fg(if is_error { Color::Red } else { Color::Cyan })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                msg,
                if is_error {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
        ]));
    }

    let status = Paragraph::new(status_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White)),
    );

    frame.render_widget(status, area);
}

/// Get border style based on whether panel is active.
fn panel_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

/// Helper to create a centered rect.
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

/// Render the node details view.
fn render_node_details(frame: &mut Frame, app: &App) {
    let area = centered_rect(90, 85, frame.area());

    // Clear the background
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        frame.area(),
    );

    if let Some(ref node_info) = app.node_info {
        let mut lines = Vec::new();

        match node_info {
            NodeInfo::Bitcoin(info) => {
                lines.extend(render_bitcoin_info(info));
            }
            NodeInfo::Lnd(info) => {
                lines.extend(render_lnd_info(info));
            }
        }

        // Add help text at the bottom - all shortcuts on the same line
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("↑↓/j/k", Style::default().fg(Color::Cyan)),
            Span::raw(": Scroll  |  "),
            Span::styled("Esc/q", Style::default().fg(Color::Red)),
            Span::raw(": Back"),
        ]));

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Node Details ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false })
            .scroll((app.node_info_scroll as u16, 0));

        frame.render_widget(paragraph, area);
    } else {
        let text = Paragraph::new("No node information available").block(
            Block::default()
                .title(" Node Details ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        );
        frame.render_widget(text, area);
    }
}

/// Render Bitcoin Core node information.
fn render_bitcoin_info(info: &BitcoinNodeInfo) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![Span::styled(
            "Bitcoin Core Node",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Version:        ", Style::default().fg(Color::Cyan)),
            Span::raw(info.version.clone()),
        ]),
        Line::from(vec![
            Span::styled("Chain:          ", Style::default().fg(Color::Cyan)),
            Span::raw(info.chain.clone()),
        ]),
        Line::from(vec![
            Span::styled("Block Height:   ", Style::default().fg(Color::Cyan)),
            Span::raw(info.blocks.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Connections:    ", Style::default().fg(Color::Cyan)),
            Span::raw(info.connections.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Difficulty:     ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{:.8}", info.difficulty)),
        ]),
        Line::from(vec![
            Span::styled("IBD Complete:   ", Style::default().fg(Color::Cyan)),
            Span::styled(
                if info.ibd_complete { "Yes" } else { "No" },
                if info.ibd_complete {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Yellow)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Wallet Balance: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{:.8} BTC", info.balance),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Network Endpoints",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("RPC:            ", Style::default().fg(Color::Cyan)),
            Span::raw(info.rpc_host.clone()),
        ]),
        Line::from(vec![
            Span::styled("P2P:            ", Style::default().fg(Color::Cyan)),
            Span::raw(info.p2p_host.clone()),
        ]),
    ]
}

/// Render LND node information.
fn render_lnd_info(info: &LndNodeInfo) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(vec![Span::styled(
            "LND Node",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Alias:          ", Style::default().fg(Color::Cyan)),
            Span::raw(info.alias.clone()),
        ]),
        Line::from(vec![
            Span::styled("Version:        ", Style::default().fg(Color::Cyan)),
            Span::raw(info.version.clone()),
        ]),
        Line::from(vec![
            Span::styled("Identity:       ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{}...", &info.identity_pubkey[..20])),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Sync Status",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Chain Synced:   ", Style::default().fg(Color::Cyan)),
            Span::styled(
                if info.synced_to_chain { "Yes" } else { "No" },
                if info.synced_to_chain {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Yellow)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Graph Synced:   ", Style::default().fg(Color::Cyan)),
            Span::styled(
                if info.synced_to_graph { "Yes" } else { "No" },
                if info.synced_to_graph {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Yellow)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("Block Height:   ", Style::default().fg(Color::Cyan)),
            Span::raw(info.block_height.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Block Hash:     ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{}...", &info.block_hash[..20])),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Network",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Peers:          ", Style::default().fg(Color::Cyan)),
            Span::raw(info.num_peers.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Active Channels:", Style::default().fg(Color::Cyan)),
            Span::raw(info.num_active_channels.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Pending Channels:", Style::default().fg(Color::Cyan)),
            Span::raw(info.num_pending_channels.to_string()),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Balances",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Wallet Balance: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{} sats", info.wallet_balance),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("Channel Balance:", Style::default().fg(Color::Cyan)),
            Span::styled(
                format!("{} sats", info.channel_balance),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Endpoints",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("REST API:       ", Style::default().fg(Color::Cyan)),
            Span::raw(info.rest_host.clone()),
        ]),
        Line::from(vec![
            Span::styled("gRPC:           ", Style::default().fg(Color::Cyan)),
            Span::raw(info.grpc_host.clone()),
        ]),
    ];

    // Add channels section if there are any channels
    if !info.channels.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            "Channels",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));

        for (idx, channel) in info.channels.iter().enumerate() {
            let status_color = if channel.active {
                Color::Green
            } else {
                Color::Red
            };
            let status = if channel.active { "Active" } else { "Inactive" };

            lines.push(Line::from(vec![Span::styled(
                format!("Channel {} ({})", idx + 1, status),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            )]));

            // Show abbreviated channel point
            let chan_point = &channel.channel_point;
            let chan_point_display = if chan_point.len() > 40 {
                format!(
                    "{}...:{}",
                    &chan_point[..37],
                    chan_point.split(':').last().unwrap_or("")
                )
            } else {
                chan_point.clone()
            };

            lines.push(Line::from(vec![
                Span::styled("  Point:        ", Style::default().fg(Color::Cyan)),
                Span::raw(chan_point_display),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Capacity:     ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{} sats", channel.capacity),
                    Style::default().fg(Color::White),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Local:        ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{} sats", channel.local_balance),
                    Style::default().fg(Color::Green),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Remote:       ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{} sats", channel.remote_balance),
                    Style::default().fg(Color::White),
                ),
            ]));
            lines.push(Line::from(""));
        }
    }

    lines
}

/// Helper function to create a form field line.
fn create_form_field<'a>(
    label: &'a str,
    value: &'a str,
    is_active: bool,
    show_cursor: bool,
) -> Line<'a> {
    let label_style = if is_active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let value_style = if is_active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let mut spans = vec![
        Span::styled(format!("{:<20}", label), label_style),
        Span::styled(value, value_style),
    ];

    if is_active && show_cursor {
        spans.push(Span::styled("_", Style::default().fg(Color::Yellow)));
    }

    Line::from(spans)
}

/// Render the mine blocks dialog.
fn render_mine_blocks(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 30, frame.area());

    let block = Block::default()
        .title(" Mine Blocks ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Number of blocks: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                &app.mine_blocks_count,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("_"),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Enter a number and press Enter to mine blocks | Esc: Cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

/// Render the fund wallet dialog.
fn render_fund_wallet(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 40, frame.area());

    let block = Block::default()
        .title(" Fund Wallet ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let node_name = app
        .nodes
        .get(app.fund_node_idx)
        .map(|s| s.as_str())
        .unwrap_or("None");

    let text = vec![
        Line::from(""),
        create_form_field("Node:", node_name, app.fund_form_field == 0, false),
        Line::from(Span::styled(
            "  (Use ← → to change)",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        create_form_field(
            "Amount (BTC):",
            &app.fund_amount,
            app.fund_form_field == 1,
            true,
        ),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Tab/↑↓: Navigate | ← →: Select node | Enter: Fund | Esc: Cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

/// Render the open channel dialog.
fn render_open_channel(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 50, frame.area());

    let block = Block::default()
        .title(" Open Lightning Channel ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let from_node = app
        .nodes
        .get(app.channel_from_idx)
        .map(|s| s.as_str())
        .unwrap_or("None");
    let to_node = app
        .nodes
        .get(app.channel_to_idx)
        .map(|s| s.as_str())
        .unwrap_or("None");

    let text = vec![
        Line::from(""),
        create_form_field("From Node:", from_node, app.channel_form_field == 0, false),
        Line::from(Span::styled(
            "  (Use ← → to change)",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        create_form_field("To Node:", to_node, app.channel_form_field == 1, false),
        Line::from(Span::styled(
            "  (Use ← → to change)",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        create_form_field(
            "Capacity (sats):",
            &app.channel_capacity,
            app.channel_form_field == 2,
            true,
        ),
        Line::from(""),
        create_form_field(
            "Push Amount (sats):",
            &app.channel_push_amount,
            app.channel_form_field == 3,
            true,
        ),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Tab/↑↓: Navigate | ← →: Select nodes | Enter: Open | Esc: Cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

/// Render the close channel dialog.
fn render_close_channel(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 40, frame.area());

    let block = Block::default()
        .title(" Close Lightning Channel ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let node_name = app
        .nodes
        .get(app.close_channel_node_idx)
        .map(|s| s.as_str())
        .unwrap_or("None");

    let force_text = if app.close_channel_force {
        "Force Close (on-chain)"
    } else {
        "Cooperative Close"
    };

    let text = vec![
        Line::from(""),
        create_form_field("Node:", node_name, app.close_channel_form_field == 0, false),
        Line::from(Span::styled(
            "  (Use ← → to change)",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        create_form_field(
            "Channel Point:",
            &app.close_channel_point,
            app.close_channel_form_field == 1,
            true,
        ),
        Line::from(Span::styled(
            "  (Format: txid:index)",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Close Type:     ",
                if app.close_channel_form_field == 2 {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Cyan)
                },
            ),
            Span::styled("< ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                force_text,
                if app.close_channel_force {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
            Span::styled(" >", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(Span::styled(
            "  (Use ← → to toggle)",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Tab/↑↓: Navigate | ← →: Change values | Enter: Close | Esc: Cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

/// Render the send payment dialog.
fn render_send_payment(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 50, frame.area());

    let block = Block::default()
        .title(" Send Lightning Payment ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let from_node = app
        .nodes
        .get(app.payment_from_idx)
        .map(|s| s.as_str())
        .unwrap_or("None");
    let to_node = app
        .nodes
        .get(app.payment_to_idx)
        .map(|s| s.as_str())
        .unwrap_or("None");

    let text = vec![
        Line::from(""),
        create_form_field("From Node:", from_node, app.payment_form_field == 0, false),
        Line::from(Span::styled(
            "  (Use ← → to change)",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        create_form_field("To Node:", to_node, app.payment_form_field == 1, false),
        Line::from(Span::styled(
            "  (Use ← → to change)",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        create_form_field(
            "Amount (sats):",
            &app.payment_amount,
            app.payment_form_field == 2,
            true,
        ),
        Line::from(""),
        create_form_field(
            "Memo:",
            &app.payment_memo,
            app.payment_form_field == 3,
            true,
        ),
        Line::from(""),
        Line::from(""),
        Line::from(Span::styled(
            "Tab/↑↓: Navigate | ← →: Select nodes | Enter: Send | Esc: Cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}
