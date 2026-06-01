use regex::Regex;
use std::sync::OnceLock;

// List of all 64 official districts in Bangladesh, including common variations/abbreviations
const DISTRICTS: &[&str] = &[
    "ঢাকা", "চট্ট", "চট্টগ্রাম", "সিলেট", "খুলনা", "রাজশাহী", "বরিশাল", "রংপুর", "ময়মনসিংহ", 
    "কুমিল্লা", "নোয়াখালী", "ফরিদপুর", "যশোর", "কুষ্টিয়া", "বগুড়া", "পাবনা", "দিনাজপুর", "টাঙ্গাইল", 
    "গাজীপুর", "নারায়ণগঞ্জ", "রাঙ্গামাটি", "বান্দরবান", "খাগড়াছড়ি", "কক্সবাজার", "ফেনী", "লক্ষ্মীপুর", 
    "চাঁদপুর", "ব্রাহ্মণবাড়ীয়া", "ব্রাহ্মণবাড়িয়া", "হবিগঞ্জ", "মৌলভীবাজার", "সুনামগঞ্জ", "নাটোর", 
    "নওগাঁ", "চাঁপাইনবাবগঞ্জ", "জয়পুরহাট", "সিরাজগঞ্জ", "গাইবান্ধা", "কুড়িগ্রাম", "লালমনিরহাট", 
    "নীলফামারী", "পঞ্চগড়", "ঠাকুরগাঁও", "বাগেরহাট", "সাতক্ষীরা", "ঝিনাইদহ", "মাগুরা", "নড়াইল", 
    "চুয়াডাঙ্গা", "মেহেরপুর", "পটুয়াখালী", "ভোলা", "পিরোজপুর", "ঝালকাঠি", "বরগুনা", "শেরপুর", 
    "নেত্রকোনা", "কিশোরগঞ্জ", "মানিকগঞ্জ", "মুন্সীগঞ্জ", "নরসিংদী", "মাদারীপুর", "শরীয়তপুর", 
    "রাজবাড়ী", "রাজবাড়ি", "গোপালগঞ্জ"
];

// Valid vehicle class letters in Bengali (ক to হ)
const CLASS_LETTERS: &[char] = &[
    'ক', 'খ', 'গ', 'ঘ', 'ঙ', 'চ', 'ছ', 'জ', 'ঝ', 'ঞ', 'ট', 'ঠ', 'ড', 'ঢ', 'ণ', 'ত', 'থ', 'দ', 'ধ', 'ন', 'প', 'ফ', 'ব', 'ভ', 'ম', 'য', 'র', 'ল', 'শ', 'ষ', 'স', 'হ'
];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ValidatedPlate {
    pub raw_text: String,
    pub top_line: String,
    pub bottom_line: String,
    pub district: String,
    pub metro: bool,
    pub class_letter: char,
    pub plate_number: String, // format: "XX-XXXX" in Bengali digits
}

/// Normalizes English digits (0-9) to Bengali digits (০-৯)
pub fn normalize_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '0' => '০',
            '1' => '১',
            '2' => '২',
            '3' => '৩',
            '4' => '৪',
            '5' => '৫',
            '6' => '৬',
            '7' => '৭',
            '8' => '৮',
            '9' => '৯',
            other => other,
        })
        .collect()
}

/// Calculates the Levenshtein distance between two strings
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let v1: Vec<char> = s1.chars().collect();
    let v2: Vec<char> = s2.chars().collect();
    let len1 = v1.len();
    let len2 = v2.len();

    let mut dp = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        dp[i][0] = i;
    }
    for j in 0..=len2 {
        dp[0][j] = j;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            if v1[i - 1] == v2[j - 1] {
                dp[i][j] = dp[i - 1][j - 1];
            } else {
                dp[i][j] = 1 + std::cmp::min(
                    dp[i - 1][j - 1], // substitution
                    std::cmp::min(
                        dp[i - 1][j], // deletion
                        dp[i][j - 1], // insertion
                    ),
                );
            }
        }
    }
    dp[len1][len2]
}

/// Corrects misspelled district names using edit distance (tolerance of 1 character)
fn correct_district(input: &str) -> Option<&'static str> {
    if DISTRICTS.contains(&input) {
        return Some(match input {
            "চট্টগ্রাম" | "চট্ট" => "চট্ট",
            "রাজবাড়ি" => "রাজবাড়ী",
            "ব্রাহ্মণবাড়ীয়া" => "ব্রাহ্মণবাড়িয়া",
            other => {
                *DISTRICTS.iter().find(|&&d| d == other).unwrap()
            }
        });
    }

    // Try fuzzy match
    let mut best_match = None;
    let mut min_dist = usize::MAX;

    for &district in DISTRICTS {
        let dist = levenshtein_distance(input, district);
        if dist < min_dist && dist <= 1 {
            min_dist = dist;
            best_match = Some(district);
        }
    }

    best_match.map(|m| match m {
        "চট্টগ্রাম" | "চট্ট" => "চট্ট",
        "রাজবাড়ি" => "রাজবাড়ী",
        "ব্রাহ্মণবাড়ীয়া" => "ব্রাহ্মণবাড়িয়া",
        other => other,
    })
}

/// Validates OCR text and extracts structured license plate details
pub fn validate_plate(raw_ai_text: &str) -> Result<ValidatedPlate, String> {
    static BOTTOM_LINE_REGEX: OnceLock<Regex> = OnceLock::new();
    let bottom_regex = BOTTOM_LINE_REGEX.get_or_init(|| {
        Regex::new(r"([০-৯]{2})\s*[-—–\s]\s*([০-৯]{4})").unwrap()
    });

    let normalized_text = normalize_digits(raw_ai_text);
    let lines: Vec<&str> = normalized_text.lines().map(|l| l.trim()).collect();

    // 1. Find the bottom line containing 6 digits with a hyphen
    let mut bottom_line_idx = None;
    let mut parsed_bottom = None;

    for (idx, line) in lines.iter().enumerate() {
        // Strip out all characters except Bengali digits and common dashes
        let cleaned_line: String = line
            .chars()
            .filter(|c| c.is_ascii_digit() || ('০'..='৯').contains(c) || *c == '-' || *c == '—' || *c == '–' || c.is_whitespace())
            .collect();

        if let Some(caps) = bottom_regex.captures(&cleaned_line) {
            let part1 = caps.get(1).unwrap().as_str();
            let part2 = caps.get(2).unwrap().as_str();
            parsed_bottom = Some(format!("{}-{}", part1, part2));
            bottom_line_idx = Some(idx);
            break;
        }
    }

    let bottom_line = parsed_bottom.ok_or_else(|| {
        "Could not find valid license plate bottom line (XX-XXXX in Bengali digits)".to_string()
    })?;

    // 2. Scan remaining lines for the top line (district, metro, class letter)
    let bottom_idx = bottom_line_idx.unwrap();
    let mut validated_top = None;

    for (idx, line) in lines.iter().enumerate() {
        if idx == bottom_idx {
            continue;
        }

        // Clean up symbols, keeping letters, spaces, and Bengali characters
        let cleaned_line: String = line
            .chars()
            .filter(|&c| {
                ((c.is_alphabetic() || (c >= '\u{0980}' && c <= '\u{09FF}'))
                    && !('০'..='৯').contains(&c)
                    && !c.is_ascii_digit())
                    || c.is_whitespace()
                    || c == '‌'
                    || c == '‍'
            })
            .collect();

        let tokens: Vec<&str> = cleaned_line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }

        // Let's check for standard patterns.
        // A top line can have 1 to 3 tokens (e.g. "ঢাকা মেট্রো ঘ" or "সিলেট হ" or "ঢাকা-মেট্রো-ঘ" merged)
        // If they are merged, let's also support parsing them.
        let mut district_name = None;
        let mut is_metro = false;
        let mut class_letter = None;

        // Helper to check if a token contains a class letter
        let find_class = |s: &str| -> Option<char> {
            s.chars().find(|c| CLASS_LETTERS.contains(c))
        };

        if tokens.len() == 3 {
            // e.g. "ঢাকা মেট্রো ঘ"
            district_name = correct_district(tokens[0]);
            is_metro = tokens[1] == "মেট্রো";
            class_letter = find_class(tokens[2]);
        } else if tokens.len() == 2 {
            // e.g. "ঢাকা ঘ" or "ঢাকা মেট্রোঘ" or "ঢাকামেট্রো ঘ"
            let tok0 = tokens[0];
            let tok1 = tokens[1];

            if tok1 == "মেট্রো" {
                // incomplete top line (missing class)
            } else if tok1.starts_with("মেট্রো") {
                district_name = correct_district(tok0);
                is_metro = true;
                class_letter = find_class(&tok1[15..]); // "মেট্রো" is 15 bytes in UTF-8
            } else if tok0.ends_with("মেট্রো") {
                let dist_part = &tok0[..tok0.len() - 15];
                district_name = correct_district(dist_part);
                is_metro = true;
                class_letter = find_class(tok1);
            } else {
                district_name = correct_district(tok0);
                class_letter = find_class(tok1);
            }
        } else if tokens.len() == 1 {
            // e.g., single merged token "ঢাকামেট্রোঘ" or "ঢাকাঘ"
            let tok = tokens[0];
            // Try to find a matching district prefix
            for &d in DISTRICTS {
                if tok.starts_with(d) {
                    district_name = Some(match d {
                        "চট্টগ্রাম" | "চট্ট" => "চট্ট",
                        other => other,
                    });
                    let rest = &tok[d.len()..];
                    if rest.starts_with("মেট্রো") {
                        is_metro = true;
                        let class_part = &rest[15..];
                        class_letter = find_class(class_part);
                    } else {
                        class_letter = find_class(rest);
                    }
                    break;
                }
            }

            // Fuzzy match fallback if merged starts with a slightly typoed district
            if district_name.is_none() {
                // If it's a single word, maybe we can slice it
                // e.g. "ডাকামেট্রোঘ" -> distance from "ঢাকা" is 1
                if tok.len() >= 6 {
                    for &d in DISTRICTS {
                        if d.len() <= tok.len() {
                            let prefix = &tok[..d.len()];
                            if levenshtein_distance(prefix, d) <= 1 {
                                district_name = correct_district(prefix);
                                let rest = &tok[d.len()..];
                                if rest.starts_with("মেট্রো") {
                                    is_metro = true;
                                    class_letter = find_class(&rest[15..]);
                                } else {
                                    class_letter = find_class(rest);
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }

        if let (Some(dist), Some(cls)) = (district_name, class_letter) {
            let formatted_top = if is_metro {
                format!("{} মেট্রো {}", dist, cls)
            } else {
                format!("{} {}", dist, cls)
            };
            validated_top = Some((
                formatted_top,
                dist.to_string(),
                is_metro,
                cls,
            ));
            break;
        }
    }

    let (top_line, district, metro, class_letter) = validated_top.ok_or_else(|| {
        "Could not find valid license plate top line (District, Class Letter, optional Metro)".to_string()
    })?;

    Ok(ValidatedPlate {
        raw_text: raw_ai_text.to_string(),
        top_line,
        bottom_line: bottom_line.clone(),
        district,
        metro,
        class_letter,
        plate_number: bottom_line,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_digits() {
        assert_eq!(normalize_digits("12-3456"), "১২-৩৪৫৬");
        assert_eq!(normalize_digits("ABC 987"), "ABC ৯৮৭");
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("ঢাকা", "ঢাকা"), 0);
        assert_eq!(levenshtein_distance("ঢাকা", "ডাকা"), 1);
        assert_eq!(levenshtein_distance("ঢাকা", "সিলেট"), 5);
    }

    #[test]
    fn test_correct_district() {
        assert_eq!(correct_district("ঢাকা"), Some("ঢাকা"));
        assert_eq!(correct_district("ডাকা"), Some("ঢাকা")); // Spelling tolerance
        assert_eq!(correct_district("চট্টগ্রাম"), Some("চট্ট")); // Normalization
        assert_eq!(correct_district("চট্ট"), Some("চট্ট"));
    }

    #[test]
    fn test_validate_plate_success() {
        let text = "LICENSE_PLATE:\nঢাকা মেট্রো ঘ\n১২-৩৪৫৬\n\nCONTEXT: Black sedan";
        let res = validate_plate(text).unwrap();
        assert_eq!(res.district, "ঢাকা");
        assert!(res.metro);
        assert_eq!(res.class_letter, 'ঘ');
        assert_eq!(res.plate_number, "১২-৩৪৫৬");

        let text_no_metro = "LICENSE_PLATE:\nসিলেট হ\n১১-২২৩৩";
        let res_no_metro = validate_plate(text_no_metro).unwrap();
        assert_eq!(res_no_metro.district, "সিলেট");
        assert!(!res_no_metro.metro);
        assert_eq!(res_no_metro.class_letter, 'হ');
        assert_eq!(res_no_metro.plate_number, "১১-২২৩৩");
    }

    #[test]
    fn test_validate_plate_noisy_and_english() {
        let text = "Plate identified:\nডাকা মেট্রো ঘ\n12-3456\nVehicle: Car";
        let res = validate_plate(text).unwrap();
        assert_eq!(res.district, "ঢাকা");
        assert!(res.metro);
        assert_eq!(res.class_letter, 'ঘ');
        assert_eq!(res.plate_number, "১২-৩৪৫৬");
    }

    #[test]
    fn test_validate_plate_merged() {
        let text = "ঢাকামেট্রোঘ\n১২-৩৪৫৬";
        let res = validate_plate(text).unwrap();
        assert_eq!(res.district, "ঢাকা");
        assert!(res.metro);
        assert_eq!(res.class_letter, 'ঘ');
        assert_eq!(res.plate_number, "১২-৩৪৫৬");
    }
}
