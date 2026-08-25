//! Terminal-prompt helpers for `--interactive` mode. No dependency on CLI schema.

pub fn prompt_line(label: &str) -> Result<String, String> {
    print!("{label}: ");
    std::io::Write::flush(&mut std::io::stdout()).map_err(|e| e.to_string())?;
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| e.to_string())?;
    Ok(input.trim().to_string())
}

pub fn prompt_required(label: &str) -> Result<String, String> {
    loop {
        let value = prompt_line(&format!("{label} (required)"))?;
        if !value.is_empty() {
            return Ok(value);
        }
        println!("  a value is required.");
    }
}

pub fn prompt_optional(label: &str) -> Result<Option<String>, String> {
    let value = prompt_line(label)?;
    Ok(if value.is_empty() { None } else { Some(value) })
}

pub fn prompt_with_default(label: &str, default: &str) -> Result<String, String> {
    let value = prompt_line(&format!("{label} [{default}]"))?;
    Ok(if value.is_empty() {
        default.to_string()
    } else {
        value
    })
}

pub fn prompt_bool_with_default(label: &str, default: bool) -> Result<bool, String> {
    let hint = if default { "Y/n" } else { "y/N" };
    loop {
        let raw = prompt_line(&format!("{label} [{hint}]"))?;
        match raw.trim().to_lowercase().as_str() {
            "" => return Ok(default),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("  please answer y or n."),
        }
    }
}

pub fn prompt_choice_with_default(
    label: &str,
    default: &str,
    choices: &[&str],
) -> Result<String, String> {
    loop {
        let raw = prompt_line(&format!("{label} [{default}] ({})", choices.join("/")))?;
        if raw.is_empty() {
            return Ok(default.to_string());
        }
        if choices.contains(&raw.as_str()) {
            return Ok(raw);
        }
        println!("  please choose one of: {}", choices.join(", "));
    }
}

pub fn prompt_parsed<T>(label: &str, default: T) -> Result<T, String>
where
    T: std::fmt::Display + std::str::FromStr,
    T::Err: std::fmt::Display,
{
    loop {
        let raw = prompt_line(&format!("{label} [{default}]"))?;
        if raw.is_empty() {
            return Ok(default);
        }
        match raw.parse::<T>() {
            Ok(value) => return Ok(value),
            Err(e) => println!("  invalid value: {e}. try again."),
        }
    }
}
