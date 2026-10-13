use image::RgbaImage;
use tauri::Manager;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("accent")
        .invoke_handler(tauri::generate_handler![set_accent_icon])
        .build()
}

fn preset_color(preset: &str) -> Option<[u8; 3]> {
    match preset {
        "purple" => Some([168, 85, 247]),
        "green" => Some([34, 197, 94]),
        "blue" => Some([59, 130, 246]),
        "orange" => Some([249, 115, 22]),
        "rose" => Some([244, 63, 94]),
        _ => None,
    }
}

fn recolor_icon(image: &mut RgbaImage, color: [u8; 3]) {
    for pixel in image.pixels_mut() {
        let [r, g, b, alpha] = pixel.0;
        let wing = r >= g && f32::from(g) > f32::from(b) * 1.25;
        let backdrop = f32::from(b) > f32::from(g) * 1.3
            && f32::from(r) > f32::from(g) * 1.3;
        if alpha == 0 || (!wing && !backdrop) {
            continue;
        }
        let shade = f32::from(r.max(g).max(b)) / 255.0;
        for channel in 0..3 {
            pixel.0[channel] = (f32::from(color[channel]) * shade).round() as u8;
        }
    }
}

#[tauri::command]
pub fn set_accent_icon<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    preset: String,
) -> Result<(), String> {
    let color = preset_color(&preset).ok_or("Unknown accent preset")?;
    let mut icon = image::load_from_memory(include_bytes!(
        "../../icons/128x128@2x.png"
    ))
    .map_err(|error| error.to_string())?
    .to_rgba8();
    recolor_icon(&mut icon, color);
    let tauri_icon = tauri::image::Image::new_owned(
        icon.as_raw().clone(),
        icon.width(),
        icon.height(),
    );
    for window in app.webview_windows().values() {
        window
            .set_icon(tauri_icon.clone())
            .map_err(|error| error.to_string())?;
    }
    #[cfg(target_os = "linux")]
    cache_linux_icon(&app, &icon)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn cache_linux_icon<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    icon: &RgbaImage,
) -> Result<(), String> {
    let data = app.path().data_dir().map_err(|error| error.to_string())?;
    let icons = data.join("icons/hicolor/256x256/apps");
    std::fs::create_dir_all(&icons).map_err(|error| error.to_string())?;
    // Tauri bundles use the kebab-cased product name for desktop Icon entries.
    for name in ["Icarus", "icarus", app.config().identifier.as_str()] {
        icon.save(icons.join(format!("{name}.png")))
            .map_err(|error| error.to_string())?;
    }
    let cache = data.join("icons/hicolor/.icon-theme.cache");
    if cache.exists() {
        std::fs::remove_file(cache).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn accepts_only_known_presets() {
        assert!(preset_color("purple").is_some());
        assert!(preset_color("../../other").is_none());
    }

    #[test]
    fn recoloring_preserves_alpha_and_neutral_detail() {
        let mut image = RgbaImage::new(3, 1);
        image.put_pixel(0, 0, Rgba([244, 233, 149, 128]));
        image.put_pixel(1, 0, Rgba([20, 20, 20, 255]));
        image.put_pixel(2, 0, Rgba([80, 40, 10, 0]));
        recolor_icon(&mut image, [59, 130, 246]);
        assert_eq!(image.get_pixel(0, 0).0[3], 128);
        assert!(image.get_pixel(0, 0).0[2] > image.get_pixel(0, 0).0[0]);
        assert_eq!(image.get_pixel(1, 0).0, [20, 20, 20, 255]);
        assert_eq!(image.get_pixel(2, 0).0, [80, 40, 10, 0]);
    }
}
