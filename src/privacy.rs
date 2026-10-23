//! Settings → Help → Privacy and security: the short version of `PRIVACY.md`, readable without a connection.

use super::*;

const PAGE: &str = "https://github.com/DoobyGrabbs/Tideline-Journal-Desktop/blob/main/PRIVACY.md";

const SECTIONS: [(&str, &[&str]); 5] = [
    (
        "In short",
        &[
            "Your diary is encrypted on this PC with a password only you know. Without the password or your recovery key it can't be read.",
            "There is no Tideline account, cloud service, advertising or tracking. The program sends nothing anywhere unless you set up sync.",
            "Sync is optional and goes to a server you run yourself. Everything is encrypted before it leaves this PC.",
            "A forgotten password can't be recovered by anyone, including the author. A recovery key is the only way back in.",
        ],
    ),
    (
        "What leaves this PC",
        &[
            "Sync: encrypted records and pictures, to the server address you enter.",
            "AI helpers: they talk only to Ollama on this PC (127.0.0.1). Nothing goes over the internet, and private entries are never sent.",
            "PDF exports are written where you choose and are not encrypted.",
            "Copy details and the QR code show the server address and the sync code. The sync code is a secret, so show it only when you need to. Your password is never in them.",
        ],
    ),
    (
        "The sync server",
        &[
            "It can see that a diary exists and its name, how many records and pictures there are, their sizes and when they changed, and the addresses of the devices that connect.",
            "It can't see your entries, titles, moods, categories, pictures or your password. It has no keys.",
            "Anyone with the sync code can add to the synced data but can't read it. Use Sync code → Manage… → Make a new code if you think it was seen.",
            "To let a new device join, the server holds your diary's keys wrapped by your password. Use a long, unusual password.",
            "The connection is plain HTTP for now. Keep sync on a network you trust, and don't put the server on the open internet.",
        ],
    ),
    (
        "On this PC",
        &[
            "The encrypted diary, its backups and a few settings that are needed before you unlock (the data folder, the accent colour, the window mode) are in the data folder. The settings contain nothing from your diary.",
            "Files you open from an entry are copied to the data folder as plain files and deleted when the diary locks or the program next starts.",
            "Auto-lock and \"hide when in the background\" protect the open window.",
        ],
    ),
    (
        "The limits",
        &[
            "While the diary is unlocked it is readable on this PC, so lock it when you step away.",
            "Anyone who has both your recovery key and your diary file can open the diary.",
            "Screenshots are allowed. The program doesn't block them.",
            "Private entries are hidden and left out of searches, Insights, exports and the AI helpers, but they are synced and encrypted like every other entry.",
        ],
    ),
];

impl DiaryApp {
    pub(crate) fn privacy_window(&mut self, ctx: &egui::Context) {
        if !self.show_privacy {
            return;
        }
        let p = palette(ctx);
        let dark = ctx.theme() == egui::Theme::Dark;
        let (mut close, mut open_page) = (false, false);
        egui::Window::new("Privacy and security")
            .title_bar(false)
            .frame(popup_frame(&p, dark))
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                close |= popup_header(ui, &p, "Privacy and security");
                egui::Frame::new().inner_margin(14).show(ui, |ui| {
                    ui.set_width(560.0);
                    egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 220.0).max(200.0)).show(ui, |ui| {
                        for (i, (title, points)) in SECTIONS.iter().enumerate() {
                            if i > 0 {
                                ui.add_space(14.0);
                            }
                            ui.label(RichText::new(*title).size(18.0).strong().color(p.title));
                            ui.add_space(4.0);
                            for point in *points {
                                ui.add(egui::Label::new(RichText::new(format!("•  {point}")).color(if i == 0 { p.ink } else { p.muted })).wrap());
                                ui.add_space(3.0);
                            }
                        }
                    });
                    ui.add_space(10.0);
                    open_page = ui.button("Read the full page, with technical details").clicked();
                });
            });
        if open_page {
            std::process::Command::new("cmd").args(["/C", "start", "", PAGE]).spawn().ok();
        }
        if close {
            self.show_privacy = false;
        }
    }
}
