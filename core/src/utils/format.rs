use crate::imports::*;

pub fn format_duration(millis: u64) -> String {
    let seconds = millis / 1000;
    let days = seconds / (24 * 60 * 60);
    let hours = (seconds / (60 * 60)) % 24;
    let minutes = (seconds / 60) % 60;
    let seconds = (seconds % 60) as f64 + (millis % 1000) as f64 / 1000.0;

    if days > 0 {
        format!("{} days {:02}:{:02}:{:02.4}", days, hours, minutes, seconds)
    } else if hours > 0 {
        format!("{:02}:{:02}:{:02.4}", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{:02}:{:02.4}", minutes, seconds)
    } else if millis > 1000 {
        format!("{:2.4} sec", seconds)
    } else {
        format!("{} msec", millis)
    }
}

pub fn format_address_string(address: impl Into<String>, range: Option<usize>) -> String {
    let address = address.into();
    let parts = address.split(':').collect::<Vec<&str>>();
    let prefix = parts[0];
    let payload = parts[1];
    let range = range.unwrap_or(6);
    let start = range;
    let finish = payload.len() - range;

    let left = &payload[0..start];
    // let center = style(&payload[start..finish]).dim();
    let right = &payload[finish..];

    format!("{prefix}:{left}....{right}")
}

pub fn format_address(address: &Address, range: Option<usize>) -> String {
    format_address_string(address, range)
}

pub fn format_partial_string(text: impl Into<String>, range: Option<usize>) -> String {
    let text: String = text.into();
    let range = range.unwrap_or(6);
    if text.len() <= range * 2 {
        text
    } else {
        let start = range;
        let finish = text.len() - range;
        let left = &text[0..start];
        let right = &text[finish..];
        format!("{left}....{right}")
    }
}

/// IPMOS (u64) to APSAK (string) with suffix layout job generator
pub fn s2kws_layout_job(
    enable: bool,
    ipmos: u64,
    network_type: &NetworkType,
    color: Color32,
    font: FontId,
) -> LayoutJob {
    let suffix = apsak_suffix(network_type);
    let style = Style::default();

    let mut layout_job = LayoutJob::default();
    if !enable {
        let sak = ipmos_to_apsak_string_with_suffix(ipmos, network_type);
        let text = RichText::new(sak).color(color).font(font.clone());
        text.append_to(
            &mut layout_job,
            &style,
            FontSelection::Default,
            Align::Center,
        );
        layout_job
    } else if ipmos == 0 {
        let transparent = color.gamma_multiply(0.25);
        let left = RichText::new("0.0").color(color).font(font.clone());
        let right = RichText::new("0000000 ")
            .color(transparent)
            .font(font.clone());
        let suffix = RichText::new(suffix).color(color).font(font);
        left.append_to(
            &mut layout_job,
            &style,
            FontSelection::Default,
            Align::Center,
        );
        right.append_to(
            &mut layout_job,
            &style,
            FontSelection::Default,
            Align::Center,
        );
        suffix.append_to(
            &mut layout_job,
            &style,
            FontSelection::Default,
            Align::Center,
        );
        layout_job
    } else {
        let transparent = color.gamma_multiply(0.05);
        let sak = ipmos_to_apsak_string_with_trailing_zeroes(ipmos);
        let mut digits = sak.chars().rev().take_while(|c| *c == '0').count();
        if digits == 8 {
            digits = 7;
        }
        let (left, right) = sak.split_at(sak.len() - digits);
        let right = right.to_string() + " ";

        let left = RichText::new(left).color(color).font(font.clone());
        let right = RichText::new(right).color(transparent).font(font.clone());
        let suffix = RichText::new(suffix).color(color).font(font);
        left.append_to(
            &mut layout_job,
            &style,
            FontSelection::Default,
            Align::Center,
        );
        right.append_to(
            &mut layout_job,
            &style,
            FontSelection::Default,
            Align::Center,
        );
        suffix.append_to(
            &mut layout_job,
            &style,
            FontSelection::Default,
            Align::Center,
        );
        layout_job
    }
}

// pub fn text_layout_job(text: &[&str]) -> LayoutJob {
//     let mut layout_job = LayoutJob::default();
//     let text_format = TextFormat {
//         valign: Align::Center,
//         ..Default::default()
//     };
//     for t in text {
//         layout_job.append(t, 0.0, text_format.clone())
//     }
//     layout_job
// }

pub fn layout_job(text: Vec<RichText>) -> LayoutJob {
    let style = Style::default();
    let mut layout_job = LayoutJob::default();
    for t in text {
        t.append_to(
            &mut layout_job,
            &style,
            FontSelection::Default,
            Align::Center,
        );
    }
    layout_job
}

pub fn format_currency(price: f64, precision: usize) -> String {
    if precision == 0 {
        price.trunc().separated_string()
    } else {
        let string = format!("{:.8}", price);
        if let Some(idx) = string.find('.') {
            let (left, right) = string.split_at(idx + 1);
            if right.len() < precision {
                let mut right = right.to_string();
                while right.len() < precision {
                    right.push('0');
                }
                separated_float!(format!("{left}{right}"))
            } else {
                let right = &right[0..precision];
                separated_float!(format!("{left}{right}"))
            }
        } else {
            price.separated_string()
        }
    }
}

pub fn format_currency_with_symbol(price: f64, precision: usize, symbol: &str) -> String {
    let price = format_currency(price, precision);
    format!("{price} {symbol}")
}

pub fn precision_from_symbol(symbol: &str) -> usize {
    match symbol {
        "sak" => 8,
        "btc" => 8,
        // "usd" => 2,
        _ => 6,
    }
}
