# Project Terminal

Modern, GPU hızlandırmalı, düşük gecikmeli ve zengin kullanıcı deneyimi sunan Linux masaüstü terminal emülatörü.

Rust ekosisteminin kararlılığı, **wgpu** ile donanımsal grafik hızlandırması ve modern TUI/CLI standartlarıyla geliştirilmiştir.

---

## Mimari ve Öne Çıkan Özellikler

### 1. GPU Hızlandırmalı Render Motoru
- **WGPU & Vulkan/OpenGL Arka Ucu:** Düşük CPU yükü ve yüksek kare hızlarında kararlı çizim.
- **Yatay Quad Kümeleme (Horizontal Batching):** Ardışık aynı renkli hücreler ve alt çizgiler tek bir quad geometrisinde birleştirilerek çizim çağrısı (draw call) sayısı en aza indirilir.
- **Glyphon & cosmic-text Metin Dizgisi:** Sub-pixel hassasiyetinde glif render'ı ve dinamik font atlası yönetimi.
- **Yaylanmalı Akıcı İmleç (Spring Lerp Cursor):** iOS tarzı akıcı, sönümlü fizik yaylanması ile pürüzsüz imleç hareketi.

### 2. Protokol ve Standart Uyumluluğu
- **Sixel Grafik Motoru:** VT340 16 renk ve Truecolor 24-bit RGB paletlerini ayrıştırır; GPU doku önbelleği (`image_cache`) ve bind group yönetimiyle sıfır fazladan bellek kopyalaması ile ekrana çizer.
- **Kitty Klavye Protokolü (Progressive Keyboard Enhancement):**
  - Modifikatörlü tuşlar (Ctrl, Shift, Alt, Super) için `CSI unicode-codepoint ; modifiers u` standart çıkışı.
  - Kabuk uyumluluğu için modifikatörsüz `Enter`, `Tab`, `Backspace` tuşlarında miras mod davranışının korunması.
  - Ana ekran tamponu ile alternatif ekran tamponu (`\x1b[?1049h/l`) arasında bağımsız klavye durum ve yığın izolasyonu.
  - Tüm tuşları bildirme modu (`Flag 8`) ve ayrıştırma modu (`Flag 1`) tam desteği.
- **DEC Mod 2026 (Synchronized Output):** TUI araçları ve editörlerde (Neovim, Tmux) atomik ekran güncellemeleri, yırtılmasız (tear-free) render.
- **OSC 8 Tıklanabilir Hiperlinkler:** Terminaldeki URL ve özel bağlantıları algılama, altı çizili vurgulama ve tıklama ile varsayılan tarayıcıda açma.
- **OSC 52 Sistem Panosu:** Uzak SSH oturumları ve yerel uygulamalardan panoya doğrudan okuma ve yazma.
- **OSC 133 Shell Entegrasyonu:** Komut başlangıcı, komut yürütme ve komut sonu blok takibi; bloklar arası klavye ile hızlı atlama.
- **Genişletilmiş ANSI/VT Desteği:** Truecolor (24-bit RGB), 256 indeksli renkler, altı çizili, kalın, italik, sönük ve ters renk nitelikleri.

### 3. PTY ve Akış Denetimi (Flow Control)
- **Linux Yerel PTY (Nix):** `openpty`, `setsid` ve `TIOCSCTTY` ile tam uyumlu Linux PTY kontrolü.
- **Çekirdek Düzeyinde Geri Basınç (Kernel Backpressure):** Sınırsız bellek tahsisi yerine `sync_channel(32)` sınırlandırmasıyla, GUI kuyruğu meşgul olduğunda üretici süreci (`cat`, `find`) çekirdek seviyesinde askıya alma ve OOM kilitlenmelerini önleme.
- **64KB Veri Paketleme:** GUI olay döngüsünü doyurmadan dengeli çıktı akışı sağlama.

### 4. Dahili Arayüz ve Çoklu Görev Yetenekleri
- **Tek Satır Birleşik Başlık Çubuğu:** Sekmeler ve pencere denetimlerini tek satırda birleştiren modern, kompakt tasarım.
- **İkili Ağaç Tabanlı Pencere Bölme (Binary Tree Split Panes):** Yatay ve dikey sınırsız bölme, oran koruma ve fare ile sürüklenebilir bölücüler.
- **Çıktı İçi Arama Çubuğu (Ctrl+Shift+F):** Canlı arama, eşleşme sayacı ve eşleşmeler arasında gezinme.
- **Dahili GPU Komut Paleti (Ctrl+Shift+P):** Klavye kısayollarını ve terminal aksiyonlarını arama ve tetikleme.
- **Gelişmiş Sağ Tık Bağlam Menüsü:** Kopyala, yapıştır, URL aç, panel böl ve kapatma aksiyonları.

---

## Modül Yapısı (Crates)

```text
crates/
├── core/      # Hücre, Grid, Flat Ring Buffer, VT Parser, Sixel Decoder, OSC Motoru
├── pty/       # Linux native PTY, process yönetimi, backpressure ve shell entegrasyonu
├── render/    # WGPU grafik boru hattı, glyphon metin atlası, Sixel texture pipeline, UI overlay
└── gui/       # Winit olay döngüsü, pencere ağacı, komut paleti, kısayollar ve uygulama mantığı
```

---

## Kurulum ve Derleme

### Gereksinimler
- Rust 1.80+ (2021 Edition)
- Vulkan veya OpenGL destekli grafik sürücüleri
- Linux kütüphaneleri (X11 / Wayland, libxkbcommon)

### Hızlı Başlangıç
```bash
# Depoyu klonlayın
git clone git@github.com:atalhatulu/project-terminal.git
cd project-terminal

# Testleri çalıştırın
cargo test --workspace

# Optimize edilmiş sürümü derleyin ve çalıştırın
cargo run --release
```

---

## Varsayılan Kısayollar

| Kısayol | Açıklama |
| :--- | :---: |
| `Ctrl + Shift + T` | Yeni Sekme Aç |
| `Ctrl + Shift + W` | Aktif Paneli / Sekmeyi Kapat |
| `Ctrl + Shift + D` | Aktif Paneli Yatay Böl |
| `Ctrl + Shift + E` | Aktif Paneli Dikey Böl |
| `Ctrl + Shift + F` | Terminal Çıktısında Arama Yap |
| `Ctrl + Shift + P` | Komut Paletini Aç |
| `Ctrl + Shift + Z` | Aktif Paneli Tam Ekrana Yakınlaştır (Zoom) |
| `Ctrl + Shift + C` | Seçili Metni Kopyala |
| `Ctrl + Shift + V` | Panodan Yapıştır |
| `Ctrl + =` / `Ctrl + -` | Font Boyutunu Büyüt / Küçült |
| `Ctrl + 0` | Font Boyutunu Sıfırla |

---

## Lisans

Bu proje [MIT](LICENSE) veya [Apache-2.0](LICENSE) şartları altında lisanslanmıştır.
