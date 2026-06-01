use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    static ref DISTRICTS: Vec<&'static str> = vec![
        "ঢাকা", "চট্ট", "সিলেট", "রাজশাহী", "খুলনা", "বরিশাল", "রংপুর", "ময়মনসিংহ", "গাজীপুর",
        "কুমিল্লা", "নারায়ণগঞ্জ"
    ];

    static ref CLASSES: Vec<&'static str> = vec![
        "ক", "খ", "গ", "ঘ", "চ", "ছ", "জ", "ঝ", "ট", "ঠ", "ড", "ঢ", "ত", "থ", "দ", "ধ", "ন",
        "প", "ফ", "ব", "ভ", "ম", "য", "র", "ল", "শ", "স", "হ"
    ];

    // Bottom line: Exactly 6 Bengali digits with an optional dash (e.g. ১২-৩৪৫৬ or ১২৩৪৫৬)
    static ref BOTTOM_LINE_RE: Regex = Regex::new(r"^[০-৯]{2}-?[০-৯]{4}$").unwrap();
}

pub fn validate_bangla_license_plate(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }

    // A common format might be on one line like "ঢাকা মেট্রো-গ ১২-৩৪৫৬"
    // or two lines where top is "ঢাকা মেট্রো-গ" and bottom is "১২-৩৪৫৬"

    // For simplicity, we'll try to find the components in the full text.
    let mut has_district = false;
    for district in DISTRICTS.iter() {
        if text.contains(district) {
            has_district = true;
            break;
        }
    }

    let mut has_class = false;
    for class in CLASSES.iter() {
        // Look for class letter with possible dashes or spaces around it, like "-গ ", " গ ", or "গ-"
        // We use Regex to ensure it's not part of another word (like the district name)
        let class_re = Regex::new(&format!(r"(?:^|[\s-])(?:{})(?:[\s-]|$)", class)).unwrap();
        if class_re.is_match(text) {
            has_class = true;
            break;
        }
    }

    // Extract the numeric part (bottom line)
    // We look for any sequence of bengali digits that matches our pattern.
    // It could be separated by space from the top line.
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut has_valid_bottom_line = false;

    for word in words {
        if BOTTOM_LINE_RE.is_match(word) {
            has_valid_bottom_line = true;
            break;
        }
    }

    has_district && has_class && has_valid_bottom_line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_plate() {
        assert!(validate_bangla_license_plate("ঢাকা মেট্রো-গ ১২-৩৪৫৬"));
        assert!(validate_bangla_license_plate("চট্ট মেট্রো-খ ১১-২২৩৩"));
    }

    #[test]
    fn test_invalid_plate() {
        assert!(!validate_bangla_license_plate("ঢাকা মেট্রো-গ ১২-৩৪৫")); // 5 digits
        assert!(!validate_bangla_license_plate("ফেক মেট্রো-গ ১২-৩৪৫৬")); // invalid district
    }
}
