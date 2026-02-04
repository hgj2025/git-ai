# UTF-8 Filename Comprehensive TDD Implementation Plan

## Bug #404: Non-ASCII Filename Handling

When filenames contain non-ASCII characters (Chinese, emoji, etc.), AI-generated code was incorrectly classified as human-written because git outputs such filenames with octal escape sequences.

## Goal

Expand the current fix for Chinese/emoji filenames to comprehensively handle ALL Unicode character types using Test-Driven Development, committing each category separately.

---

## Characters/Languages That Can Break Filename Handling

| Category | Examples | Risk Level |
|----------|----------|------------|
| **RTL Scripts** | Arabic (العربية), Hebrew (עברית), Persian, Urdu | 🔴 High - bidirectional marks |
| **Indic Scripts** | Hindi (हिंदी), Tamil (தமிழ்), Bengali (বাংলা), Telugu | 🔴 High - combining vowels |
| **Southeast Asian** | Thai (ภาษาไทย), Vietnamese (tiếng việt), Khmer, Lao | 🟡 Medium - tone marks |
| **CJK Extended** | Japanese (ひらがな/カタカナ), Korean (한글) | 🟢 Low - similar to Chinese |
| **Cyrillic/Greek** | Russian (Русский), Greek (Ελληνικά) | 🟢 Low |
| **Zero-width chars** | ZWSP, ZWJ, ZWNJ, bidi marks | 🔴 High - invisible |
| **Emoji sequences** | 👨‍👩‍👧‍👦 (ZWJ), 👋🏽 (skin tone), 🇺🇸 (flag) | 🟡 Medium |
| **Normalization** | NFC vs NFD (macOS uses NFD) | 🔴 High - same file, different bytes |
| **Special symbols** | Mathematical (∑∫√), Currency (€£¥₹₿) | 🟢 Low |

---

## Implementation Phases

### Phase 1: CJK Extended Coverage (Japanese, Korean, Traditional Chinese)
**Tests:**
- `test_japanese_hiragana_katakana_filename()` - ひらがな, カタカナ
- `test_japanese_kanji_filename()` - 漢字
- `test_korean_hangul_filename()` - 한글.txt
- `test_chinese_traditional_filename()` - 繁體中文.txt
- `test_mixed_cjk_filename()` - 日本語中文한글.txt

**Unit tests in `src/utils.rs`:**
- `test_unescape_japanese()`, `test_unescape_korean()`

**Commit:** `test: Add CJK extended coverage tests (Japanese, Korean, Traditional Chinese) #404`

---

### Phase 2: RTL Scripts (Arabic, Hebrew, Persian, Urdu)
**Tests:**
- `test_arabic_filename()` - مرحبا.txt
- `test_hebrew_filename()` - שלום.txt
- `test_persian_filename()` - فارسی.txt
- `test_urdu_filename()` - اردو.txt
- `test_rtl_with_ltr_mixed_filename()` - test_مرحبا_file.txt
- `test_bidirectional_marks_in_filename()` - invisible LTR/RTL marks

**Commit:** `test: Add RTL scripts tests (Arabic, Hebrew, Persian, Urdu) #404`

---

### Phase 3: Indic Scripts (Hindi, Tamil, Bengali, Telugu)
**Tests:**
- `test_hindi_devanagari_filename()` - हिंदी.txt
- `test_tamil_filename()` - தமிழ்.txt
- `test_bengali_filename()` - বাংলা.txt
- `test_telugu_filename()` - తెలుగు.txt
- `test_devanagari_combining_chars()` - combining vowel marks

**Commit:** `test: Add Indic scripts tests (Hindi, Tamil, Bengali, Telugu) #404`

---

### Phase 4: Southeast Asian Scripts (Thai, Vietnamese, Khmer, Lao)
**Tests:**
- `test_thai_filename()` - ภาษาไทย.txt
- `test_vietnamese_filename()` - tiếng_việt.txt (with tone marks)
- `test_khmer_filename()` - ភាសាខ្មែរ.txt
- `test_lao_filename()` - ພາສາລາວ.txt

**Commit:** `test: Add Southeast Asian scripts tests (Thai, Vietnamese, Khmer, Lao) #404`

---

### Phase 5: Cyrillic and Greek Scripts
**Tests:**
- `test_russian_cyrillic_filename()` - Русский.txt
- `test_ukrainian_cyrillic_filename()` - Українська.txt
- `test_greek_filename()` - Ελληνικά.txt
- `test_greek_polytonic_filename()` - Ἑλληνική.txt

**Commit:** `test: Add Cyrillic and Greek scripts tests #404`

---

### Phase 6: Extended Emoji (ZWJ, skin tones, flags)
**Tests:**
- `test_emoji_with_skin_tone_modifiers()` - 👋🏽
- `test_emoji_zwj_sequences()` - 👨‍👩‍👧‍👦 (family)
- `test_emoji_flag_sequences()` - 🇺🇸 (flag)
- `test_emoji_keycap_sequences()` - 1️⃣

**Commit:** `test: Add extended emoji tests (ZWJ, skin tones, flags) #404`

---

### Phase 7: Special Unicode (zero-width, math, currency)
**Tests:**
- `test_zero_width_characters()` - ZWSP, ZWJ, ZWNJ
- `test_mathematical_symbols()` - ∑, ∫, √
- `test_currency_symbols()` - €, £, ¥, ₹, ₿
- `test_box_drawing_characters()` - ┌─┐│└┘

**Commit:** `test: Add special Unicode characters tests (zero-width, math, currency) #404`

---

### Phase 8: Unicode Normalization (NFC/NFD)
**Tests:**
- `test_nfc_nfd_equivalence()` - café (NFC) vs café (NFD)
- `test_macos_nfd_normalization()` - macOS-specific test (conditional)
- `test_hangul_nfc_nfd()` - Korean normalization

**Commit:** `test: Add Unicode normalization tests (NFC/NFD) #404`

---

### Phase 9: Edge cases and stress tests
**Tests:**
- `test_very_long_utf8_filename()` - 255-byte limit
- `test_deeply_nested_utf8_directories()` - src/日本/中国/한국/
- `test_many_utf8_files_in_commit()` - Multiple UTF-8 files
- `test_filename_with_all_unicode_categories()` - Mix of all scripts

**Commit:** `test: Add edge cases and stress tests for UTF-8 filenames #404`

---

## Testing Commands

```bash
# Run UTF-8 specific tests
cargo test utf8_filenames

# Run with verbose output
cargo test utf8_filenames -- --nocapture

# Run all tests
cargo test
```

## Key Implementation Notes

1. **Use existing `unescape_git_path()` function** - Already handles octal escaping correctly
2. **Prefer `-z` flag** where possible for NUL-byte separation in git commands
3. **Add normalization only if macOS tests fail** - Avoid unnecessary dependencies
4. **Group tests by script family** for organization and clarity
