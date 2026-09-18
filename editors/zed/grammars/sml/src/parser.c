#include "tree_sitter/parser.h"

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic ignored "-Wmissing-field-initializers"
#endif

#define LANGUAGE_VERSION 14
#define STATE_COUNT 30
#define LARGE_STATE_COUNT 11
#define SYMBOL_COUNT 32
#define ALIAS_COUNT 0
#define TOKEN_COUNT 18
#define EXTERNAL_TOKEN_COUNT 0
#define FIELD_COUNT 2
#define MAX_ALIAS_SEQUENCE_LENGTH 3
#define PRODUCTION_ID_COUNT 4

enum ts_symbol_identifiers {
  sym_word = 1,
  anon_sym_COLON = 2,
  anon_sym_LBRACE = 3,
  anon_sym_COMMA = 4,
  anon_sym_RBRACE = 5,
  anon_sym_LBRACK = 6,
  anon_sym_RBRACK = 7,
  sym_comment = 8,
  anon_sym_DQUOTE = 9,
  aux_sym_string_token1 = 10,
  sym_escape_sequence = 11,
  sym_number = 12,
  sym_boolean = 13,
  sym_null = 14,
  sym_env_var = 15,
  sym_fragment_ref = 16,
  sym_directive = 17,
  sym_source_file = 18,
  sym__item = 19,
  sym_pair = 20,
  sym_key = 21,
  sym_block = 22,
  sym_array = 23,
  sym_directive_phrase = 24,
  sym__directive_arg = 25,
  sym_type_name = 26,
  sym__value = 27,
  sym_string = 28,
  aux_sym_source_file_repeat1 = 29,
  aux_sym_block_repeat1 = 30,
  aux_sym_string_repeat1 = 31,
};

static const char * const ts_symbol_names[] = {
  [ts_builtin_sym_end] = "end",
  [sym_word] = "word",
  [anon_sym_COLON] = ":",
  [anon_sym_LBRACE] = "{",
  [anon_sym_COMMA] = ",",
  [anon_sym_RBRACE] = "}",
  [anon_sym_LBRACK] = "[",
  [anon_sym_RBRACK] = "]",
  [sym_comment] = "comment",
  [anon_sym_DQUOTE] = "\"",
  [aux_sym_string_token1] = "string_token1",
  [sym_escape_sequence] = "escape_sequence",
  [sym_number] = "number",
  [sym_boolean] = "boolean",
  [sym_null] = "null",
  [sym_env_var] = "env_var",
  [sym_fragment_ref] = "fragment_ref",
  [sym_directive] = "directive",
  [sym_source_file] = "source_file",
  [sym__item] = "_item",
  [sym_pair] = "pair",
  [sym_key] = "key",
  [sym_block] = "block",
  [sym_array] = "array",
  [sym_directive_phrase] = "directive_phrase",
  [sym__directive_arg] = "_directive_arg",
  [sym_type_name] = "type_name",
  [sym__value] = "_value",
  [sym_string] = "string",
  [aux_sym_source_file_repeat1] = "source_file_repeat1",
  [aux_sym_block_repeat1] = "block_repeat1",
  [aux_sym_string_repeat1] = "string_repeat1",
};

static const TSSymbol ts_symbol_map[] = {
  [ts_builtin_sym_end] = ts_builtin_sym_end,
  [sym_word] = sym_word,
  [anon_sym_COLON] = anon_sym_COLON,
  [anon_sym_LBRACE] = anon_sym_LBRACE,
  [anon_sym_COMMA] = anon_sym_COMMA,
  [anon_sym_RBRACE] = anon_sym_RBRACE,
  [anon_sym_LBRACK] = anon_sym_LBRACK,
  [anon_sym_RBRACK] = anon_sym_RBRACK,
  [sym_comment] = sym_comment,
  [anon_sym_DQUOTE] = anon_sym_DQUOTE,
  [aux_sym_string_token1] = aux_sym_string_token1,
  [sym_escape_sequence] = sym_escape_sequence,
  [sym_number] = sym_number,
  [sym_boolean] = sym_boolean,
  [sym_null] = sym_null,
  [sym_env_var] = sym_env_var,
  [sym_fragment_ref] = sym_fragment_ref,
  [sym_directive] = sym_directive,
  [sym_source_file] = sym_source_file,
  [sym__item] = sym__item,
  [sym_pair] = sym_pair,
  [sym_key] = sym_key,
  [sym_block] = sym_block,
  [sym_array] = sym_array,
  [sym_directive_phrase] = sym_directive_phrase,
  [sym__directive_arg] = sym__directive_arg,
  [sym_type_name] = sym_type_name,
  [sym__value] = sym__value,
  [sym_string] = sym_string,
  [aux_sym_source_file_repeat1] = aux_sym_source_file_repeat1,
  [aux_sym_block_repeat1] = aux_sym_block_repeat1,
  [aux_sym_string_repeat1] = aux_sym_string_repeat1,
};

static const TSSymbolMetadata ts_symbol_metadata[] = {
  [ts_builtin_sym_end] = {
    .visible = false,
    .named = true,
  },
  [sym_word] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_COLON] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LBRACE] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_COMMA] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RBRACE] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LBRACK] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RBRACK] = {
    .visible = true,
    .named = false,
  },
  [sym_comment] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_DQUOTE] = {
    .visible = true,
    .named = false,
  },
  [aux_sym_string_token1] = {
    .visible = false,
    .named = false,
  },
  [sym_escape_sequence] = {
    .visible = true,
    .named = true,
  },
  [sym_number] = {
    .visible = true,
    .named = true,
  },
  [sym_boolean] = {
    .visible = true,
    .named = true,
  },
  [sym_null] = {
    .visible = true,
    .named = true,
  },
  [sym_env_var] = {
    .visible = true,
    .named = true,
  },
  [sym_fragment_ref] = {
    .visible = true,
    .named = true,
  },
  [sym_directive] = {
    .visible = true,
    .named = true,
  },
  [sym_source_file] = {
    .visible = true,
    .named = true,
  },
  [sym__item] = {
    .visible = false,
    .named = true,
  },
  [sym_pair] = {
    .visible = true,
    .named = true,
  },
  [sym_key] = {
    .visible = true,
    .named = true,
  },
  [sym_block] = {
    .visible = true,
    .named = true,
  },
  [sym_array] = {
    .visible = true,
    .named = true,
  },
  [sym_directive_phrase] = {
    .visible = true,
    .named = true,
  },
  [sym__directive_arg] = {
    .visible = false,
    .named = true,
  },
  [sym_type_name] = {
    .visible = true,
    .named = true,
  },
  [sym__value] = {
    .visible = false,
    .named = true,
  },
  [sym_string] = {
    .visible = true,
    .named = true,
  },
  [aux_sym_source_file_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_block_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_string_repeat1] = {
    .visible = false,
    .named = false,
  },
};

enum ts_field_identifiers {
  field_key = 1,
  field_value = 2,
};

static const char * const ts_field_names[] = {
  [0] = NULL,
  [field_key] = "key",
  [field_value] = "value",
};

static const TSFieldMapSlice ts_field_map_slices[PRODUCTION_ID_COUNT] = {
  [1] = {.index = 0, .length = 1},
  [2] = {.index = 1, .length = 2},
  [3] = {.index = 3, .length = 2},
};

static const TSFieldMapEntry ts_field_map_entries[] = {
  [0] =
    {field_key, 0},
  [1] =
    {field_key, 0},
    {field_value, 1},
  [3] =
    {field_key, 0},
    {field_value, 2},
};

static const TSSymbol ts_alias_sequences[PRODUCTION_ID_COUNT][MAX_ALIAS_SEQUENCE_LENGTH] = {
  [0] = {0},
};

static const uint16_t ts_non_terminal_alias_map[] = {
  0,
};

static const TSStateId ts_primary_state_ids[STATE_COUNT] = {
  [0] = 0,
  [1] = 1,
  [2] = 2,
  [3] = 3,
  [4] = 4,
  [5] = 5,
  [6] = 6,
  [7] = 7,
  [8] = 8,
  [9] = 9,
  [10] = 10,
  [11] = 11,
  [12] = 12,
  [13] = 13,
  [14] = 14,
  [15] = 15,
  [16] = 16,
  [17] = 17,
  [18] = 18,
  [19] = 19,
  [20] = 20,
  [21] = 21,
  [22] = 22,
  [23] = 23,
  [24] = 24,
  [25] = 25,
  [26] = 26,
  [27] = 27,
  [28] = 28,
  [29] = 29,
};

static TSCharacterRange sym_directive_character_set_1[] = {
  {0, 0x08}, {0x0e, 0x1f}, {'!', '!'}, {'$', '+'}, {'-', '9'}, {';', 'Z'}, {'\\', '\\'}, {'^', 'z'},
  {'|', '|'}, {'~', 0x10ffff},
};

static bool ts_lex(TSLexer *lexer, TSStateId state) {
  START_LEXER();
  eof = lexer->eof(lexer);
  switch (state) {
    case 0:
      if (eof) ADVANCE(19);
      ADVANCE_MAP(
        '"', 35,
        '#', 34,
        '$', 71,
        '&', 84,
        '+', 70,
        ',', 22,
        '-', 68,
        '/', 61,
        '0', 49,
        ':', 20,
        '@', 86,
        '[', 24,
        '\\', 60,
        ']', 25,
        '_', 65,
        '{', 21,
        '}', 23,
      );
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ') SKIP(17);
      if (('1' <= lookahead && lookahead <= '9')) ADVANCE(52);
      if (lookahead != 0) ADVANCE(87);
      END_STATE();
    case 1:
      if (lookahead == '"') ADVANCE(35);
      if (lookahead == '#') ADVANCE(27);
      if (lookahead == '-') ADVANCE(44);
      if (lookahead == '/') ADVANCE(37);
      if (lookahead == '\\') ADVANCE(10);
      if (lookahead == '_') ADVANCE(41);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ') ADVANCE(36);
      if (lookahead != 0) ADVANCE(45);
      END_STATE();
    case 2:
      if (lookahead == '*') ADVANCE(4);
      if (lookahead == '/') ADVANCE(34);
      END_STATE();
    case 3:
      if (lookahead == '*') ADVANCE(3);
      if (lookahead == '/') ADVANCE(26);
      if (lookahead != 0) ADVANCE(4);
      END_STATE();
    case 4:
      if (lookahead == '*') ADVANCE(3);
      if (lookahead != 0) ADVANCE(4);
      END_STATE();
    case 5:
      if (lookahead == '*') ADVANCE(7);
      if (lookahead != 0) ADVANCE(5);
      END_STATE();
    case 6:
      if (lookahead == '*') ADVANCE(5);
      END_STATE();
    case 7:
      if (lookahead == '*') ADVANCE(8);
      if (lookahead == '_') ADVANCE(26);
      if (lookahead != 0) ADVANCE(5);
      END_STATE();
    case 8:
      if (lookahead == '*') ADVANCE(8);
      if (lookahead == '_') ADVANCE(29);
      if (lookahead != 0) ADVANCE(5);
      END_STATE();
    case 9:
      if (lookahead == '-') ADVANCE(34);
      END_STATE();
    case 10:
      if (lookahead == 'u') ADVANCE(11);
      if (lookahead == '"' ||
          lookahead == '0' ||
          lookahead == '\\' ||
          lookahead == 'n' ||
          lookahead == 'r' ||
          lookahead == 't') ADVANCE(46);
      END_STATE();
    case 11:
      if (lookahead == '{') ADVANCE(14);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(16);
      END_STATE();
    case 12:
      if (lookahead == '}') ADVANCE(46);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(12);
      END_STATE();
    case 13:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(46);
      END_STATE();
    case 14:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(12);
      END_STATE();
    case 15:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(13);
      END_STATE();
    case 16:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(15);
      END_STATE();
    case 17:
      if (eof) ADVANCE(19);
      ADVANCE_MAP(
        '"', 35,
        '#', 34,
        '$', 71,
        '&', 84,
        '+', 70,
        ',', 22,
        '-', 68,
        '/', 61,
        '0', 49,
        ':', 20,
        '@', 86,
        '[', 24,
        ']', 25,
        '_', 65,
        '{', 21,
        '}', 23,
      );
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ') SKIP(17);
      if (('1' <= lookahead && lookahead <= '9')) ADVANCE(52);
      if (lookahead != 0) ADVANCE(87);
      END_STATE();
    case 18:
      if (eof) ADVANCE(19);
      if (lookahead == '#') ADVANCE(34);
      if (lookahead == '-') ADVANCE(9);
      if (lookahead == '/') ADVANCE(2);
      if (lookahead == ':') ADVANCE(20);
      if (lookahead == '_') ADVANCE(6);
      if (lookahead == '{') ADVANCE(21);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ') SKIP(18);
      END_STATE();
    case 19:
      ACCEPT_TOKEN(ts_builtin_sym_end);
      END_STATE();
    case 20:
      ACCEPT_TOKEN(anon_sym_COLON);
      END_STATE();
    case 21:
      ACCEPT_TOKEN(anon_sym_LBRACE);
      END_STATE();
    case 22:
      ACCEPT_TOKEN(anon_sym_COMMA);
      END_STATE();
    case 23:
      ACCEPT_TOKEN(anon_sym_RBRACE);
      END_STATE();
    case 24:
      ACCEPT_TOKEN(anon_sym_LBRACK);
      END_STATE();
    case 25:
      ACCEPT_TOKEN(anon_sym_RBRACK);
      END_STATE();
    case 26:
      ACCEPT_TOKEN(sym_comment);
      END_STATE();
    case 27:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead == '\n') ADVANCE(45);
      if (lookahead == '"' ||
          lookahead == '\\') ADVANCE(34);
      if (lookahead != 0) ADVANCE(27);
      END_STATE();
    case 28:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead == '*') ADVANCE(66);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == '"' ||
          lookahead == '#' ||
          lookahead == ',' ||
          lookahead == ':' ||
          lookahead == '[' ||
          lookahead == ']' ||
          lookahead == '{' ||
          lookahead == '}') ADVANCE(5);
      if (lookahead != 0) ADVANCE(64);
      END_STATE();
    case 29:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead == '*') ADVANCE(7);
      if (lookahead != 0) ADVANCE(5);
      END_STATE();
    case 30:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead == '*') ADVANCE(42);
      if (lookahead == '"' ||
          lookahead == '\\') ADVANCE(5);
      if (lookahead != 0) ADVANCE(40);
      END_STATE();
    case 31:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead == '\t' ||
          (0x0b <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == '"' ||
          lookahead == '#' ||
          lookahead == ',' ||
          lookahead == ':' ||
          lookahead == '[' ||
          lookahead == ']' ||
          lookahead == '{' ||
          lookahead == '}') ADVANCE(34);
      if (lookahead != 0 &&
          (lookahead < '\t' || '\r' < lookahead)) ADVANCE(31);
      END_STATE();
    case 32:
      ACCEPT_TOKEN(sym_comment);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 33:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead != 0 &&
          lookahead != '"' &&
          lookahead != '\\') ADVANCE(45);
      END_STATE();
    case 34:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(34);
      END_STATE();
    case 35:
      ACCEPT_TOKEN(anon_sym_DQUOTE);
      END_STATE();
    case 36:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead == '#') ADVANCE(27);
      if (lookahead == '-') ADVANCE(44);
      if (lookahead == '/') ADVANCE(37);
      if (lookahead == '_') ADVANCE(41);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ') ADVANCE(36);
      if (lookahead != 0 &&
          lookahead != '"' &&
          lookahead != '#' &&
          lookahead != '\\') ADVANCE(45);
      END_STATE();
    case 37:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead == '*') ADVANCE(39);
      if (lookahead == '/') ADVANCE(27);
      if (lookahead != 0 &&
          lookahead != '"' &&
          lookahead != '\\') ADVANCE(45);
      END_STATE();
    case 38:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead == '*') ADVANCE(38);
      if (lookahead == '/') ADVANCE(33);
      if (lookahead == '"' ||
          lookahead == '\\') ADVANCE(4);
      if (lookahead != 0) ADVANCE(39);
      END_STATE();
    case 39:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead == '*') ADVANCE(38);
      if (lookahead == '"' ||
          lookahead == '\\') ADVANCE(4);
      if (lookahead != 0) ADVANCE(39);
      END_STATE();
    case 40:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead == '*') ADVANCE(42);
      if (lookahead == '"' ||
          lookahead == '\\') ADVANCE(5);
      if (lookahead != 0) ADVANCE(40);
      END_STATE();
    case 41:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead == '*') ADVANCE(40);
      if (lookahead != 0 &&
          lookahead != '"' &&
          lookahead != '\\') ADVANCE(45);
      END_STATE();
    case 42:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead == '*') ADVANCE(43);
      if (lookahead == '_') ADVANCE(33);
      if (lookahead == '"' ||
          lookahead == '\\') ADVANCE(5);
      if (lookahead != 0) ADVANCE(40);
      END_STATE();
    case 43:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead == '*') ADVANCE(43);
      if (lookahead == '_') ADVANCE(30);
      if (lookahead == '"' ||
          lookahead == '\\') ADVANCE(5);
      if (lookahead != 0) ADVANCE(40);
      END_STATE();
    case 44:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead == '-') ADVANCE(27);
      if (lookahead != 0 &&
          lookahead != '"' &&
          lookahead != '\\') ADVANCE(45);
      END_STATE();
    case 45:
      ACCEPT_TOKEN(aux_sym_string_token1);
      if (lookahead != 0 &&
          lookahead != '"' &&
          lookahead != '\\') ADVANCE(45);
      END_STATE();
    case 46:
      ACCEPT_TOKEN(sym_escape_sequence);
      END_STATE();
    case 47:
      ACCEPT_TOKEN(sym_escape_sequence);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 48:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == '.') ADVANCE(78);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(79);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(48);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(53);
      if (('A' <= lookahead && lookahead <= 'F') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(54);
      END_STATE();
    case 49:
      ACCEPT_TOKEN(sym_number);
      ADVANCE_MAP(
        '.', 78,
        'B', 76,
        'b', 76,
        'E', 75,
        'e', 75,
        'O', 77,
        'o', 77,
        'X', 83,
        'x', 83,
      );
      if (('0' <= lookahead && lookahead <= '9') ||
          lookahead == '_') ADVANCE(52);
      END_STATE();
    case 50:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == '.') ADVANCE(78);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(75);
      if (lookahead == '0' ||
          lookahead == '1' ||
          lookahead == '_') ADVANCE(50);
      END_STATE();
    case 51:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == '.') ADVANCE(78);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(75);
      if (('0' <= lookahead && lookahead <= '7') ||
          lookahead == '_') ADVANCE(51);
      END_STATE();
    case 52:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == '.') ADVANCE(78);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(75);
      if (('0' <= lookahead && lookahead <= '9') ||
          lookahead == '_') ADVANCE(52);
      END_STATE();
    case 53:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == '.') ADVANCE(78);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(48);
      if (('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(54);
      if (('0' <= lookahead && lookahead <= '9') ||
          lookahead == '_') ADVANCE(53);
      END_STATE();
    case 54:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == '.') ADVANCE(78);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(48);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(54);
      END_STATE();
    case 55:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(75);
      if (('0' <= lookahead && lookahead <= '9') ||
          lookahead == '_') ADVANCE(55);
      END_STATE();
    case 56:
      ACCEPT_TOKEN(sym_number);
      if (('0' <= lookahead && lookahead <= '9') ||
          lookahead == '_') ADVANCE(56);
      END_STATE();
    case 57:
      ACCEPT_TOKEN(sym_env_var);
      if (lookahead == '-' ||
          lookahead == '.' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(57);
      END_STATE();
    case 58:
      ACCEPT_TOKEN(sym_fragment_ref);
      if (lookahead == '-' ||
          lookahead == '.' ||
          ('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(58);
      END_STATE();
    case 59:
      ACCEPT_TOKEN(sym_directive);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(59);
      END_STATE();
    case 60:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '"') ADVANCE(46);
      if (lookahead == 'u') ADVANCE(74);
      if (lookahead == '0' ||
          lookahead == '\\' ||
          lookahead == 'n' ||
          lookahead == 'r' ||
          lookahead == 't') ADVANCE(47);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 61:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '*') ADVANCE(63);
      if (lookahead == '/') ADVANCE(31);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 62:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '*') ADVANCE(62);
      if (lookahead == '/') ADVANCE(32);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == '"' ||
          lookahead == '#' ||
          lookahead == ',' ||
          lookahead == ':' ||
          lookahead == '[' ||
          lookahead == ']' ||
          lookahead == '{' ||
          lookahead == '}') ADVANCE(4);
      if (lookahead != 0) ADVANCE(63);
      END_STATE();
    case 63:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '*') ADVANCE(62);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == '"' ||
          lookahead == '#' ||
          lookahead == ',' ||
          lookahead == ':' ||
          lookahead == '[' ||
          lookahead == ']' ||
          lookahead == '{' ||
          lookahead == '}') ADVANCE(4);
      if (lookahead != 0) ADVANCE(63);
      END_STATE();
    case 64:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '*') ADVANCE(66);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == '"' ||
          lookahead == '#' ||
          lookahead == ',' ||
          lookahead == ':' ||
          lookahead == '[' ||
          lookahead == ']' ||
          lookahead == '{' ||
          lookahead == '}') ADVANCE(5);
      if (lookahead != 0) ADVANCE(64);
      END_STATE();
    case 65:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '*') ADVANCE(64);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 66:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '*') ADVANCE(67);
      if (lookahead == '_') ADVANCE(32);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == '"' ||
          lookahead == '#' ||
          lookahead == ',' ||
          lookahead == ':' ||
          lookahead == '[' ||
          lookahead == ']' ||
          lookahead == '{' ||
          lookahead == '}') ADVANCE(5);
      if (lookahead != 0) ADVANCE(64);
      END_STATE();
    case 67:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '*') ADVANCE(67);
      if (lookahead == '_') ADVANCE(28);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ' ||
          lookahead == '"' ||
          lookahead == '#' ||
          lookahead == ',' ||
          lookahead == ':' ||
          lookahead == '[' ||
          lookahead == ']' ||
          lookahead == '{' ||
          lookahead == '}') ADVANCE(5);
      if (lookahead != 0) ADVANCE(64);
      END_STATE();
    case 68:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '-') ADVANCE(31);
      if (lookahead == '0') ADVANCE(49);
      if (('1' <= lookahead && lookahead <= '9')) ADVANCE(52);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 69:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '.') ADVANCE(85);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 70:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '0') ADVANCE(49);
      if (('1' <= lookahead && lookahead <= '9')) ADVANCE(52);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 71:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == 'e') ADVANCE(72);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 72:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == 'n') ADVANCE(73);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 73:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == 'v') ADVANCE(69);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 74:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '{') ADVANCE(14);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(82);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 75:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(79);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(56);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 76:
      ACCEPT_TOKEN(sym_word);
      if (lookahead == '0' ||
          lookahead == '1' ||
          lookahead == '_') ADVANCE(50);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 77:
      ACCEPT_TOKEN(sym_word);
      if (('0' <= lookahead && lookahead <= '7') ||
          lookahead == '_') ADVANCE(51);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 78:
      ACCEPT_TOKEN(sym_word);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(55);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 79:
      ACCEPT_TOKEN(sym_word);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(56);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 80:
      ACCEPT_TOKEN(sym_word);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(47);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 81:
      ACCEPT_TOKEN(sym_word);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(80);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 82:
      ACCEPT_TOKEN(sym_word);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(81);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 83:
      ACCEPT_TOKEN(sym_word);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(54);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 84:
      ACCEPT_TOKEN(sym_word);
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(58);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 85:
      ACCEPT_TOKEN(sym_word);
      if (('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(57);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    case 86:
      ACCEPT_TOKEN(sym_word);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(59);
      END_STATE();
    case 87:
      ACCEPT_TOKEN(sym_word);
      if ((!eof && set_contains(sym_directive_character_set_1, 10, lookahead))) ADVANCE(87);
      END_STATE();
    default:
      return false;
  }
}

static bool ts_lex_keywords(TSLexer *lexer, TSStateId state) {
  START_LEXER();
  eof = lexer->eof(lexer);
  switch (state) {
    case 0:
      if (lookahead == 'f') ADVANCE(1);
      if (lookahead == 'n') ADVANCE(2);
      if (lookahead == 't') ADVANCE(3);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ') SKIP(0);
      END_STATE();
    case 1:
      if (lookahead == 'a') ADVANCE(4);
      END_STATE();
    case 2:
      if (lookahead == 'u') ADVANCE(5);
      END_STATE();
    case 3:
      if (lookahead == 'r') ADVANCE(6);
      END_STATE();
    case 4:
      if (lookahead == 'l') ADVANCE(7);
      END_STATE();
    case 5:
      if (lookahead == 'l') ADVANCE(8);
      END_STATE();
    case 6:
      if (lookahead == 'u') ADVANCE(9);
      END_STATE();
    case 7:
      if (lookahead == 's') ADVANCE(10);
      END_STATE();
    case 8:
      if (lookahead == 'l') ADVANCE(11);
      END_STATE();
    case 9:
      if (lookahead == 'e') ADVANCE(12);
      END_STATE();
    case 10:
      if (lookahead == 'e') ADVANCE(12);
      END_STATE();
    case 11:
      ACCEPT_TOKEN(sym_null);
      END_STATE();
    case 12:
      ACCEPT_TOKEN(sym_boolean);
      END_STATE();
    default:
      return false;
  }
}

static const TSLexMode ts_lex_modes[STATE_COUNT] = {
  [0] = {.lex_state = 0},
  [1] = {.lex_state = 17},
  [2] = {.lex_state = 17},
  [3] = {.lex_state = 17},
  [4] = {.lex_state = 17},
  [5] = {.lex_state = 17},
  [6] = {.lex_state = 17},
  [7] = {.lex_state = 17},
  [8] = {.lex_state = 17},
  [9] = {.lex_state = 17},
  [10] = {.lex_state = 17},
  [11] = {.lex_state = 17},
  [12] = {.lex_state = 17},
  [13] = {.lex_state = 17},
  [14] = {.lex_state = 17},
  [15] = {.lex_state = 17},
  [16] = {.lex_state = 17},
  [17] = {.lex_state = 17},
  [18] = {.lex_state = 17},
  [19] = {.lex_state = 17},
  [20] = {.lex_state = 17},
  [21] = {.lex_state = 17},
  [22] = {.lex_state = 17},
  [23] = {.lex_state = 17},
  [24] = {.lex_state = 17},
  [25] = {.lex_state = 1},
  [26] = {.lex_state = 1},
  [27] = {.lex_state = 1},
  [28] = {.lex_state = 18},
  [29] = {.lex_state = 18},
};

static const uint16_t ts_parse_table[LARGE_STATE_COUNT][SYMBOL_COUNT] = {
  [0] = {
    [ts_builtin_sym_end] = ACTIONS(1),
    [sym_word] = ACTIONS(1),
    [anon_sym_COLON] = ACTIONS(1),
    [anon_sym_LBRACE] = ACTIONS(1),
    [anon_sym_COMMA] = ACTIONS(1),
    [anon_sym_RBRACE] = ACTIONS(1),
    [anon_sym_LBRACK] = ACTIONS(1),
    [anon_sym_RBRACK] = ACTIONS(1),
    [sym_comment] = ACTIONS(3),
    [anon_sym_DQUOTE] = ACTIONS(1),
    [sym_escape_sequence] = ACTIONS(1),
    [sym_number] = ACTIONS(1),
    [sym_boolean] = ACTIONS(1),
    [sym_null] = ACTIONS(1),
    [sym_env_var] = ACTIONS(1),
    [sym_fragment_ref] = ACTIONS(1),
    [sym_directive] = ACTIONS(1),
  },
  [1] = {
    [sym_source_file] = STATE(29),
    [sym__item] = STATE(8),
    [sym_pair] = STATE(8),
    [sym_key] = STATE(28),
    [sym_block] = STATE(20),
    [sym_array] = STATE(20),
    [sym_directive_phrase] = STATE(8),
    [sym__value] = STATE(8),
    [sym_string] = STATE(13),
    [aux_sym_source_file_repeat1] = STATE(8),
    [ts_builtin_sym_end] = ACTIONS(5),
    [sym_word] = ACTIONS(7),
    [anon_sym_LBRACE] = ACTIONS(9),
    [anon_sym_LBRACK] = ACTIONS(11),
    [sym_comment] = ACTIONS(3),
    [anon_sym_DQUOTE] = ACTIONS(13),
    [sym_number] = ACTIONS(15),
    [sym_boolean] = ACTIONS(15),
    [sym_null] = ACTIONS(15),
    [sym_env_var] = ACTIONS(15),
    [sym_fragment_ref] = ACTIONS(15),
    [sym_directive] = ACTIONS(17),
  },
  [2] = {
    [sym__item] = STATE(2),
    [sym_pair] = STATE(2),
    [sym_key] = STATE(28),
    [sym_block] = STATE(20),
    [sym_array] = STATE(20),
    [sym_directive_phrase] = STATE(2),
    [sym__value] = STATE(2),
    [sym_string] = STATE(13),
    [aux_sym_block_repeat1] = STATE(2),
    [sym_word] = ACTIONS(19),
    [anon_sym_LBRACE] = ACTIONS(22),
    [anon_sym_COMMA] = ACTIONS(25),
    [anon_sym_RBRACE] = ACTIONS(28),
    [anon_sym_LBRACK] = ACTIONS(30),
    [anon_sym_RBRACK] = ACTIONS(28),
    [sym_comment] = ACTIONS(3),
    [anon_sym_DQUOTE] = ACTIONS(33),
    [sym_number] = ACTIONS(25),
    [sym_boolean] = ACTIONS(25),
    [sym_null] = ACTIONS(25),
    [sym_env_var] = ACTIONS(25),
    [sym_fragment_ref] = ACTIONS(25),
    [sym_directive] = ACTIONS(36),
  },
  [3] = {
    [sym__item] = STATE(5),
    [sym_pair] = STATE(5),
    [sym_key] = STATE(28),
    [sym_block] = STATE(20),
    [sym_array] = STATE(20),
    [sym_directive_phrase] = STATE(5),
    [sym__value] = STATE(5),
    [sym_string] = STATE(13),
    [aux_sym_block_repeat1] = STATE(5),
    [sym_word] = ACTIONS(7),
    [anon_sym_LBRACE] = ACTIONS(9),
    [anon_sym_COMMA] = ACTIONS(39),
    [anon_sym_RBRACE] = ACTIONS(41),
    [anon_sym_LBRACK] = ACTIONS(11),
    [sym_comment] = ACTIONS(3),
    [anon_sym_DQUOTE] = ACTIONS(13),
    [sym_number] = ACTIONS(39),
    [sym_boolean] = ACTIONS(39),
    [sym_null] = ACTIONS(39),
    [sym_env_var] = ACTIONS(39),
    [sym_fragment_ref] = ACTIONS(39),
    [sym_directive] = ACTIONS(17),
  },
  [4] = {
    [sym__item] = STATE(6),
    [sym_pair] = STATE(6),
    [sym_key] = STATE(28),
    [sym_block] = STATE(20),
    [sym_array] = STATE(20),
    [sym_directive_phrase] = STATE(6),
    [sym__value] = STATE(6),
    [sym_string] = STATE(13),
    [aux_sym_block_repeat1] = STATE(6),
    [sym_word] = ACTIONS(7),
    [anon_sym_LBRACE] = ACTIONS(9),
    [anon_sym_COMMA] = ACTIONS(43),
    [anon_sym_LBRACK] = ACTIONS(11),
    [anon_sym_RBRACK] = ACTIONS(45),
    [sym_comment] = ACTIONS(3),
    [anon_sym_DQUOTE] = ACTIONS(13),
    [sym_number] = ACTIONS(43),
    [sym_boolean] = ACTIONS(43),
    [sym_null] = ACTIONS(43),
    [sym_env_var] = ACTIONS(43),
    [sym_fragment_ref] = ACTIONS(43),
    [sym_directive] = ACTIONS(17),
  },
  [5] = {
    [sym__item] = STATE(2),
    [sym_pair] = STATE(2),
    [sym_key] = STATE(28),
    [sym_block] = STATE(20),
    [sym_array] = STATE(20),
    [sym_directive_phrase] = STATE(2),
    [sym__value] = STATE(2),
    [sym_string] = STATE(13),
    [aux_sym_block_repeat1] = STATE(2),
    [sym_word] = ACTIONS(7),
    [anon_sym_LBRACE] = ACTIONS(9),
    [anon_sym_COMMA] = ACTIONS(47),
    [anon_sym_RBRACE] = ACTIONS(49),
    [anon_sym_LBRACK] = ACTIONS(11),
    [sym_comment] = ACTIONS(3),
    [anon_sym_DQUOTE] = ACTIONS(13),
    [sym_number] = ACTIONS(47),
    [sym_boolean] = ACTIONS(47),
    [sym_null] = ACTIONS(47),
    [sym_env_var] = ACTIONS(47),
    [sym_fragment_ref] = ACTIONS(47),
    [sym_directive] = ACTIONS(17),
  },
  [6] = {
    [sym__item] = STATE(2),
    [sym_pair] = STATE(2),
    [sym_key] = STATE(28),
    [sym_block] = STATE(20),
    [sym_array] = STATE(20),
    [sym_directive_phrase] = STATE(2),
    [sym__value] = STATE(2),
    [sym_string] = STATE(13),
    [aux_sym_block_repeat1] = STATE(2),
    [sym_word] = ACTIONS(7),
    [anon_sym_LBRACE] = ACTIONS(9),
    [anon_sym_COMMA] = ACTIONS(47),
    [anon_sym_LBRACK] = ACTIONS(11),
    [anon_sym_RBRACK] = ACTIONS(51),
    [sym_comment] = ACTIONS(3),
    [anon_sym_DQUOTE] = ACTIONS(13),
    [sym_number] = ACTIONS(47),
    [sym_boolean] = ACTIONS(47),
    [sym_null] = ACTIONS(47),
    [sym_env_var] = ACTIONS(47),
    [sym_fragment_ref] = ACTIONS(47),
    [sym_directive] = ACTIONS(17),
  },
  [7] = {
    [sym__item] = STATE(7),
    [sym_pair] = STATE(7),
    [sym_key] = STATE(28),
    [sym_block] = STATE(20),
    [sym_array] = STATE(20),
    [sym_directive_phrase] = STATE(7),
    [sym__value] = STATE(7),
    [sym_string] = STATE(13),
    [aux_sym_source_file_repeat1] = STATE(7),
    [ts_builtin_sym_end] = ACTIONS(53),
    [sym_word] = ACTIONS(55),
    [anon_sym_LBRACE] = ACTIONS(58),
    [anon_sym_LBRACK] = ACTIONS(61),
    [sym_comment] = ACTIONS(3),
    [anon_sym_DQUOTE] = ACTIONS(64),
    [sym_number] = ACTIONS(67),
    [sym_boolean] = ACTIONS(67),
    [sym_null] = ACTIONS(67),
    [sym_env_var] = ACTIONS(67),
    [sym_fragment_ref] = ACTIONS(67),
    [sym_directive] = ACTIONS(70),
  },
  [8] = {
    [sym__item] = STATE(7),
    [sym_pair] = STATE(7),
    [sym_key] = STATE(28),
    [sym_block] = STATE(20),
    [sym_array] = STATE(20),
    [sym_directive_phrase] = STATE(7),
    [sym__value] = STATE(7),
    [sym_string] = STATE(13),
    [aux_sym_source_file_repeat1] = STATE(7),
    [ts_builtin_sym_end] = ACTIONS(73),
    [sym_word] = ACTIONS(7),
    [anon_sym_LBRACE] = ACTIONS(9),
    [anon_sym_LBRACK] = ACTIONS(11),
    [sym_comment] = ACTIONS(3),
    [anon_sym_DQUOTE] = ACTIONS(13),
    [sym_number] = ACTIONS(75),
    [sym_boolean] = ACTIONS(75),
    [sym_null] = ACTIONS(75),
    [sym_env_var] = ACTIONS(75),
    [sym_fragment_ref] = ACTIONS(75),
    [sym_directive] = ACTIONS(17),
  },
  [9] = {
    [sym_block] = STATE(17),
    [sym__directive_arg] = STATE(14),
    [sym_type_name] = STATE(14),
    [sym_string] = STATE(14),
    [ts_builtin_sym_end] = ACTIONS(77),
    [sym_word] = ACTIONS(79),
    [anon_sym_LBRACE] = ACTIONS(9),
    [anon_sym_COMMA] = ACTIONS(77),
    [anon_sym_RBRACE] = ACTIONS(77),
    [anon_sym_LBRACK] = ACTIONS(77),
    [anon_sym_RBRACK] = ACTIONS(77),
    [sym_comment] = ACTIONS(3),
    [anon_sym_DQUOTE] = ACTIONS(13),
    [sym_number] = ACTIONS(77),
    [sym_boolean] = ACTIONS(77),
    [sym_null] = ACTIONS(77),
    [sym_env_var] = ACTIONS(81),
    [sym_fragment_ref] = ACTIONS(81),
    [sym_directive] = ACTIONS(77),
  },
  [10] = {
    [sym_block] = STATE(24),
    [sym_array] = STATE(24),
    [sym__value] = STATE(24),
    [sym_string] = STATE(24),
    [ts_builtin_sym_end] = ACTIONS(83),
    [sym_word] = ACTIONS(85),
    [anon_sym_LBRACE] = ACTIONS(9),
    [anon_sym_COMMA] = ACTIONS(83),
    [anon_sym_RBRACE] = ACTIONS(83),
    [anon_sym_LBRACK] = ACTIONS(11),
    [anon_sym_RBRACK] = ACTIONS(83),
    [sym_comment] = ACTIONS(3),
    [anon_sym_DQUOTE] = ACTIONS(13),
    [sym_number] = ACTIONS(87),
    [sym_boolean] = ACTIONS(87),
    [sym_null] = ACTIONS(87),
    [sym_env_var] = ACTIONS(87),
    [sym_fragment_ref] = ACTIONS(87),
    [sym_directive] = ACTIONS(83),
  },
};

static const uint16_t ts_small_parse_table[] = {
  [0] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(91), 1,
      sym_word,
    ACTIONS(89), 14,
      ts_builtin_sym_end,
      anon_sym_COLON,
      anon_sym_LBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [23] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(95), 1,
      sym_word,
    ACTIONS(93), 14,
      ts_builtin_sym_end,
      anon_sym_COLON,
      anon_sym_LBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [46] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(99), 1,
      sym_word,
    ACTIONS(101), 1,
      anon_sym_COLON,
    ACTIONS(103), 1,
      anon_sym_LBRACE,
    ACTIONS(97), 12,
      ts_builtin_sym_end,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [73] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(9), 1,
      anon_sym_LBRACE,
    ACTIONS(108), 1,
      sym_word,
    STATE(23), 1,
      sym_block,
    ACTIONS(106), 12,
      ts_builtin_sym_end,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [100] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(112), 1,
      sym_word,
    ACTIONS(110), 13,
      ts_builtin_sym_end,
      anon_sym_LBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [122] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(116), 1,
      sym_word,
    ACTIONS(114), 13,
      ts_builtin_sym_end,
      anon_sym_LBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [144] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(108), 1,
      sym_word,
    ACTIONS(106), 13,
      ts_builtin_sym_end,
      anon_sym_LBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [166] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(120), 1,
      sym_word,
    ACTIONS(118), 13,
      ts_builtin_sym_end,
      anon_sym_LBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [188] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(124), 1,
      sym_word,
    ACTIONS(122), 13,
      ts_builtin_sym_end,
      anon_sym_LBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [210] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(129), 1,
      sym_word,
    ACTIONS(126), 13,
      ts_builtin_sym_end,
      anon_sym_LBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [232] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(134), 1,
      sym_word,
    ACTIONS(132), 13,
      ts_builtin_sym_end,
      anon_sym_LBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [254] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(138), 1,
      sym_word,
    ACTIONS(136), 13,
      ts_builtin_sym_end,
      anon_sym_LBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [276] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(142), 1,
      sym_word,
    ACTIONS(140), 13,
      ts_builtin_sym_end,
      anon_sym_LBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [298] = 3,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(146), 1,
      sym_word,
    ACTIONS(144), 13,
      ts_builtin_sym_end,
      anon_sym_LBRACE,
      anon_sym_COMMA,
      anon_sym_RBRACE,
      anon_sym_LBRACK,
      anon_sym_RBRACK,
      anon_sym_DQUOTE,
      sym_number,
      sym_boolean,
      sym_null,
      sym_env_var,
      sym_fragment_ref,
      sym_directive,
  [320] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(148), 1,
      anon_sym_DQUOTE,
    ACTIONS(150), 1,
      aux_sym_string_token1,
    ACTIONS(152), 1,
      sym_escape_sequence,
    STATE(27), 1,
      aux_sym_string_repeat1,
  [336] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(154), 1,
      anon_sym_DQUOTE,
    ACTIONS(156), 1,
      aux_sym_string_token1,
    ACTIONS(158), 1,
      sym_escape_sequence,
    STATE(25), 1,
      aux_sym_string_repeat1,
  [352] = 5,
    ACTIONS(3), 1,
      sym_comment,
    ACTIONS(160), 1,
      anon_sym_DQUOTE,
    ACTIONS(162), 1,
      aux_sym_string_token1,
    ACTIONS(165), 1,
      sym_escape_sequence,
    STATE(27), 1,
      aux_sym_string_repeat1,
  [368] = 4,
    ACTIONS(9), 1,
      anon_sym_LBRACE,
    ACTIONS(168), 1,
      anon_sym_COLON,
    ACTIONS(170), 1,
      sym_comment,
    STATE(15), 1,
      sym_block,
  [381] = 2,
    ACTIONS(170), 1,
      sym_comment,
    ACTIONS(172), 1,
      ts_builtin_sym_end,
};

static const uint32_t ts_small_parse_table_map[] = {
  [SMALL_STATE(11)] = 0,
  [SMALL_STATE(12)] = 23,
  [SMALL_STATE(13)] = 46,
  [SMALL_STATE(14)] = 73,
  [SMALL_STATE(15)] = 100,
  [SMALL_STATE(16)] = 122,
  [SMALL_STATE(17)] = 144,
  [SMALL_STATE(18)] = 166,
  [SMALL_STATE(19)] = 188,
  [SMALL_STATE(20)] = 210,
  [SMALL_STATE(21)] = 232,
  [SMALL_STATE(22)] = 254,
  [SMALL_STATE(23)] = 276,
  [SMALL_STATE(24)] = 298,
  [SMALL_STATE(25)] = 320,
  [SMALL_STATE(26)] = 336,
  [SMALL_STATE(27)] = 352,
  [SMALL_STATE(28)] = 368,
  [SMALL_STATE(29)] = 381,
};

static const TSParseActionEntry ts_parse_actions[] = {
  [0] = {.entry = {.count = 0, .reusable = false}},
  [1] = {.entry = {.count = 1, .reusable = false}}, RECOVER(),
  [3] = {.entry = {.count = 1, .reusable = false}}, SHIFT_EXTRA(),
  [5] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_source_file, 0, 0, 0),
  [7] = {.entry = {.count = 1, .reusable = false}}, SHIFT(13),
  [9] = {.entry = {.count = 1, .reusable = true}}, SHIFT(3),
  [11] = {.entry = {.count = 1, .reusable = true}}, SHIFT(4),
  [13] = {.entry = {.count = 1, .reusable = true}}, SHIFT(26),
  [15] = {.entry = {.count = 1, .reusable = true}}, SHIFT(8),
  [17] = {.entry = {.count = 1, .reusable = true}}, SHIFT(9),
  [19] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_block_repeat1, 2, 0, 0), SHIFT_REPEAT(13),
  [22] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_block_repeat1, 2, 0, 0), SHIFT_REPEAT(3),
  [25] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_block_repeat1, 2, 0, 0), SHIFT_REPEAT(2),
  [28] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_block_repeat1, 2, 0, 0),
  [30] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_block_repeat1, 2, 0, 0), SHIFT_REPEAT(4),
  [33] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_block_repeat1, 2, 0, 0), SHIFT_REPEAT(26),
  [36] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_block_repeat1, 2, 0, 0), SHIFT_REPEAT(9),
  [39] = {.entry = {.count = 1, .reusable = true}}, SHIFT(5),
  [41] = {.entry = {.count = 1, .reusable = true}}, SHIFT(19),
  [43] = {.entry = {.count = 1, .reusable = true}}, SHIFT(6),
  [45] = {.entry = {.count = 1, .reusable = true}}, SHIFT(18),
  [47] = {.entry = {.count = 1, .reusable = true}}, SHIFT(2),
  [49] = {.entry = {.count = 1, .reusable = true}}, SHIFT(21),
  [51] = {.entry = {.count = 1, .reusable = true}}, SHIFT(22),
  [53] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0),
  [55] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(13),
  [58] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(3),
  [61] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(4),
  [64] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(26),
  [67] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(7),
  [70] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(9),
  [73] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_source_file, 1, 0, 0),
  [75] = {.entry = {.count = 1, .reusable = true}}, SHIFT(7),
  [77] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_directive_phrase, 1, 0, 0),
  [79] = {.entry = {.count = 1, .reusable = false}}, SHIFT(16),
  [81] = {.entry = {.count = 1, .reusable = true}}, SHIFT(14),
  [83] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_pair, 2, 0, 1),
  [85] = {.entry = {.count = 1, .reusable = false}}, SHIFT(24),
  [87] = {.entry = {.count = 1, .reusable = true}}, SHIFT(24),
  [89] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_string, 2, 0, 0),
  [91] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_string, 2, 0, 0),
  [93] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_string, 3, 0, 0),
  [95] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_string, 3, 0, 0),
  [97] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym__value, 1, 0, 0),
  [99] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym__value, 1, 0, 0),
  [101] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_key, 1, 0, 0),
  [103] = {.entry = {.count = 2, .reusable = true}}, REDUCE(sym_key, 1, 0, 0), REDUCE(sym__value, 1, 0, 0),
  [106] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_directive_phrase, 2, 0, 0),
  [108] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_directive_phrase, 2, 0, 0),
  [110] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_pair, 2, 0, 2),
  [112] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_pair, 2, 0, 2),
  [114] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_type_name, 1, 0, 0),
  [116] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_type_name, 1, 0, 0),
  [118] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_array, 2, 0, 0),
  [120] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_array, 2, 0, 0),
  [122] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_block, 2, 0, 0),
  [124] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_block, 2, 0, 0),
  [126] = {.entry = {.count = 2, .reusable = true}}, REDUCE(sym__item, 1, 0, 0), REDUCE(sym__value, 1, 0, 0),
  [129] = {.entry = {.count = 2, .reusable = false}}, REDUCE(sym__item, 1, 0, 0), REDUCE(sym__value, 1, 0, 0),
  [132] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_block, 3, 0, 0),
  [134] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_block, 3, 0, 0),
  [136] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_array, 3, 0, 0),
  [138] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_array, 3, 0, 0),
  [140] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_directive_phrase, 3, 0, 0),
  [142] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_directive_phrase, 3, 0, 0),
  [144] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_pair, 3, 0, 3),
  [146] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_pair, 3, 0, 3),
  [148] = {.entry = {.count = 1, .reusable = false}}, SHIFT(12),
  [150] = {.entry = {.count = 1, .reusable = false}}, SHIFT(27),
  [152] = {.entry = {.count = 1, .reusable = true}}, SHIFT(27),
  [154] = {.entry = {.count = 1, .reusable = false}}, SHIFT(11),
  [156] = {.entry = {.count = 1, .reusable = false}}, SHIFT(25),
  [158] = {.entry = {.count = 1, .reusable = true}}, SHIFT(25),
  [160] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_string_repeat1, 2, 0, 0),
  [162] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_string_repeat1, 2, 0, 0), SHIFT_REPEAT(27),
  [165] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_string_repeat1, 2, 0, 0), SHIFT_REPEAT(27),
  [168] = {.entry = {.count = 1, .reusable = true}}, SHIFT(10),
  [170] = {.entry = {.count = 1, .reusable = true}}, SHIFT_EXTRA(),
  [172] = {.entry = {.count = 1, .reusable = true}},  ACCEPT_INPUT(),
};

#ifdef __cplusplus
extern "C" {
#endif
#ifdef TREE_SITTER_HIDE_SYMBOLS
#define TS_PUBLIC
#elif defined(_WIN32)
#define TS_PUBLIC __declspec(dllexport)
#else
#define TS_PUBLIC __attribute__((visibility("default")))
#endif

TS_PUBLIC const TSLanguage *tree_sitter_sml(void) {
  static const TSLanguage language = {
    .version = LANGUAGE_VERSION,
    .symbol_count = SYMBOL_COUNT,
    .alias_count = ALIAS_COUNT,
    .token_count = TOKEN_COUNT,
    .external_token_count = EXTERNAL_TOKEN_COUNT,
    .state_count = STATE_COUNT,
    .large_state_count = LARGE_STATE_COUNT,
    .production_id_count = PRODUCTION_ID_COUNT,
    .field_count = FIELD_COUNT,
    .max_alias_sequence_length = MAX_ALIAS_SEQUENCE_LENGTH,
    .parse_table = &ts_parse_table[0][0],
    .small_parse_table = ts_small_parse_table,
    .small_parse_table_map = ts_small_parse_table_map,
    .parse_actions = ts_parse_actions,
    .symbol_names = ts_symbol_names,
    .field_names = ts_field_names,
    .field_map_slices = ts_field_map_slices,
    .field_map_entries = ts_field_map_entries,
    .symbol_metadata = ts_symbol_metadata,
    .public_symbol_map = ts_symbol_map,
    .alias_map = ts_non_terminal_alias_map,
    .alias_sequences = &ts_alias_sequences[0][0],
    .lex_modes = ts_lex_modes,
    .lex_fn = ts_lex,
    .keyword_lex_fn = ts_lex_keywords,
    .keyword_capture_token = sym_word,
    .primary_state_ids = ts_primary_state_ids,
  };
  return &language;
}
#ifdef __cplusplus
}
#endif
