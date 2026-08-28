use eframe::wasm_bindgen::{self, prelude::*};

struct App;

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Hello from broccolli!");
        });
    }
}

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");

    let canvas = document
        .get_element_by_id("the_canvas_id")
        .expect("missing #the_canvas_id canvas")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("#the_canvas_id is not a canvas element");

    eframe::WebRunner::new()
        .start(canvas, eframe::WebOptions::default(), Box::new(|_cc| Ok(Box::new(App))))
        .await
}
