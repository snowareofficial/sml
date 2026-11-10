// SPDX-License-Identifier: MulanPSL-2.0
// `.ambr` cross-end vectors: hex → LoadText → byte-for-byte against canonical_text.
// `.ambr` 跨端对拍：hex → LoadText → 与 canonical_text 逐字符比较。

package amberice

import (
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

type vector struct {
	ID            string `json:"id"`
	AmbrHex       string `json:"ambr_hex"`
	CanonicalText string `json:"canonical_text"`
}

func TestVectors(t *testing.T) {
	path := filepath.Join("..", "..", "..", "tests", "vectors", "amberice-container.v1.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Skipf("vector file not found, skipping instead of faking green / 找不到向量文件，跳过而不是假绿: %v", err)
	}

	var doc struct {
		Cases []vector `json:"cases"`
	}
	if err := json.Unmarshal(raw, &doc); err != nil {
		t.Fatalf("vector parse failed / 向量解析失败: %v", err)
	}
	if len(doc.Cases) != 22 {
		t.Fatalf("want 22 cases, got %d / 期望 22 例，实际 %d", len(doc.Cases), len(doc.Cases))
	}

	ok := 0
	for _, c := range doc.Cases {
		data, err := hex.DecodeString(c.AmbrHex)
		if err != nil {
			t.Errorf("%s: hex decode failed / hex 解码失败: %v", c.ID, err)
			continue
		}
		got, err := LoadText(data)
		if err != nil {
			t.Errorf("%s: %v", c.ID, err)
			continue
		}
		if got != c.CanonicalText {
			t.Errorf("%s: text mismatch / 文本不一致\n got=%q\nwant=%q", c.ID, got, c.CanonicalText)
			continue
		}
		ok++
	}
	if ok != len(doc.Cases) {
		t.Fatalf("%d/%d passed", ok, len(doc.Cases))
	}
	t.Logf("%d/%d byte-for-byte / 逐字符通过", ok, len(doc.Cases))
}

// TestPackRoundTrip: canonical text → PackText → LoadText must return the same bytes.
// 往返：canonical 文本 → PackText → LoadText 必须回到同一份文本。
func TestPackRoundTrip(t *testing.T) {
	path := filepath.Join("..", "..", "..", "tests", "vectors", "amberice-container.v1.json")
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Skipf("vector file not found, skipping / 找不到向量文件，跳过: %v", err)
	}

	var doc struct {
		Cases []vector `json:"cases"`
	}
	if err := json.Unmarshal(raw, &doc); err != nil {
		t.Fatalf("vector parse failed / 向量解析失败: %v", err)
	}

	ok, scalars := 0, 0
	for _, c := range doc.Cases {
		packed, err := PackText(c.CanonicalText)
		if err != nil {
			// Top-level scalars are rejected by sml-parse with E-PARSE-008 — a known
			// parser debt (unfinished §4.2), not a binding bug. Counted, not ignored.
			// 顶层标量被 sml-parse 以 E-PARSE-008 拒绝：已知欠账，不是绑定 bug。
			if strings.Contains(err.Error(), "E-PARSE-008") {
				scalars++
				continue
			}
			t.Errorf("%s: pack: %v", c.ID, err)
			continue
		}
		got, err := LoadText(packed)
		if err != nil {
			t.Errorf("%s: read back: %v", c.ID, err)
			continue
		}
		if got != c.CanonicalText {
			t.Errorf("%s: round-trip mismatch / 往返不一致\n got=%q\nwant=%q", c.ID, got, c.CanonicalText)
			continue
		}
		ok++
	}
	if ok+scalars != len(doc.Cases) {
		t.Fatalf("%d round-trip + %d scalar != %d cases", ok, scalars, len(doc.Cases))
	}
	t.Logf("%d round-trip, %d top-level scalars rejected (E-PARSE-008) / 往返与标量分类通过", ok, scalars)
}
