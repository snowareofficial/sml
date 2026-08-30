/*
 * sml_rs.cpp — sml_rs.hpp 的实现 (桥接 Rust cdylib 符号)。
 * 注意: 仅声明外部 C 符号, 不实现解析逻辑。
 */
#include "sml_rs.hpp"
#include <cstring>
#include <sstream>

extern "C" {
    char *sml_parse(const char *text);
    char *sml_dump(const char *json);
    char *sml_parse_ex(const char *text, const char *opts_json);
    char *sml_parse_file(const char *path);
    char *sml_features(void);
    char *sml_version(void);
    void sml_free(char *p);
}

namespace sml {

void CStrDeleter::operator()(char *p) const {
    if (p) sml_free(p);
}

static std::string take(char *raw) {
    if (!raw) return {};
    std::string s(raw);
    sml_free(raw);
    return s;
}

std::string ParseOptions::to_json() const {
    std::ostringstream os;
    os << "{";
    bool first = true;
    if (!features.empty()) {
        os << "\"features\":[";
        for (size_t i = 0; i < features.size(); ++i) {
            if (i) os << ",";
            os << "\"" << features[i] << "\"";
        }
        os << "]";
        first = false;
    }
    if (!env.empty()) {
        if (!first) os << ",";
        os << "\"env\":{";
        for (size_t i = 0; i < env.size(); ++i) {
            if (i) os << ",";
            auto eq = env[i].find('=');
            std::string k = eq == std::string::npos ? env[i] : env[i].substr(0, eq);
            std::string v = eq == std::string::npos ? std::string() : env[i].substr(eq + 1);
            os << "\"" << k << "\":\"" << v << "\"";
        }
        os << "}";
        first = false;
    }
    if (!allow.empty()) {
        if (!first) os << ",";
        os << "\"allow\":[";
        for (size_t i = 0; i < allow.size(); ++i) {
            if (i) os << ",";
            os << "\"" << allow[i] << "\"";
        }
        os << "]";
    }
    os << "}";
    return os.str();
}

std::string parse_ex(const std::string &text, const ParseOptions &opts) {
    std::string o = opts.to_json();
    return take(sml_parse_ex(text.c_str(), o.c_str()));
}

std::string parse_file(const std::string &path) {
    return take(sml_parse_file(path.c_str()));
}

std::string parse(const std::string &text) {
    return take(sml_parse(text.c_str()));
}

std::string dump(const std::string &json) {
    return take(sml_dump(json.c_str()));
}

std::vector<std::string> features() {
    std::string raw = take(sml_features());
    // 极简解析 ["a","b",...]
    std::vector<std::string> out;
    size_t i = raw.find('[');
    if (i == std::string::npos) return out;
    i++;
    while (i < raw.size()) {
        if (raw[i] == '"') {
            size_t j = raw.find('"', i + 1);
            if (j == std::string::npos) break;
            out.push_back(raw.substr(i + 1, j - i - 1));
            i = j + 1;
        } else {
            ++i;
        }
    }
    return out;
}

std::string version() {
    return take(sml_version());
}

} // namespace sml
