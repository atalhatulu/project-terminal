# Project Terminal Kapsamlı Audit ve Karşılaştırmalı Kıyaslama Raporu

**Hedef Proje:** `/home/teha/Documents/GitHub/project-terminal`  
**Yığın (Stack):** Rust 2021, wgpu 24.0, winit 0.30, glyphon 0.8 / cosmic-text 0.12, nix 0.29 (PTY), vte 0.14  
**Kıyaslanan Popüler Rakipler:** Alacritty, Kitty, WezTerm, Ghostty, Foot  

---

## 1. Yönetici Özeti

Project Terminal, modern Rust ekosistemi üzerine kurulu, wgpu ile GPU hızlandırmalı bir Linux masaüstü terminal emülatörüdür. Alacritty'nin yalın (sekmesiz, bölmesiz) tasarım çizgisi ile Kitty ve WezTerm'in zengin son kullanıcı özellikleri arasında bir denge kurmayı hedefler.

Mimarisinde Rust dilinin bellek güvenliği garantilerini, wgpu ile modern grafik arka uç soyutlamasını (Vulkan/OpenGL), glyphon ve cosmic-text ile tipografik metin dizgisini ve nix kütüphanesi ile Linux yerel PTY yönetimini birleştirir.

---

## 2. Kıyaslama ve Skorlama Matrisi (10 Üzerinden)

| Boyut / Kriter | Puan | Durum ve Rakiplere Kıyasla Seviye |
| :--- | :---: | :--- |
| **Protokol ve Standartlar** | **8.5 / 10** | Temel ANSI/VT, 24-bit Truecolor, OSC 8 (FIFO), OSC 52, OSC 133, Sixel grafikler, Kitty Keyboard (Flag 1, 2, 8), DEC Mod 2026 ve DECSTR desteği. *Kitty ve Ghostty seviyesine yaklaştı, Alacritty'den zengin.* |
| **Throughput ve PTY Akış Denetimi** | **8.0 / 10** | Bounded `sync_channel(32)` ve 64KB paketleme ile OOM/kilitlenme engelleniyor. Sabit tamponlama ile okuma tahsisat baskısı minimize edildi. *Foot ve Alacritty seviyesinde kararlı.* |
| **GPU Render Hattı** | **8.2 / 10** | Yatay quad batching, sıfır kopyalı `&[Cell]` satır referansı, Sixel doku önbelleği ve HiDPI `ScaleFactorChanged` entegrasyonu. *Gereksiz kare başı heap tahsisi sıfırlandı.* |
| **Dahili Özellik ve UI Zenginliği** | **8.8 / 10** | İkili ağaç tabanlı split paneller, sürüklenebilir bölücüler, dahili komut paleti, arama çubuğu, bağlam menüsü, yaylanmalı imleç animasyonu. *Alacritty ve Foot'tan açık ara önde; Kitty ve WezTerm ile doğrudan rekabetçi.* |
| **Masaüstü ve Platform Entegrasyonu** | **7.5 / 10** | HiDPI desteği, font fallback zinciri, ZWJ/Unicode birleştirme ve Linux masaüstü entegrasyon betikleri. |
| **GENEL PUAN** | **8.2 / 10** | **Emekleme aşamasını geride bırakmış, çekirdek motoru optimize edilmiş, zengin arayüzlü ve kararlı modern terminal emülatörü.** |

---

## 3. Rakiplerle Karşılaştırma Matrisi

| Özellik / Yetenek | Project Terminal | Alacritty | Kitty | WezTerm | Ghostty | Foot |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **GPU Render Motoru** | wgpu (Vulkan/GL) | OpenGL | OpenGL | wgpu / Software | Metal / OpenGL | Yazılımsal (Pixman) |
| **Pencere Bölme (Split Panes)** | Dahili (İkili Ağaç) | Yok (Tmux şart) | Dahili | Dahili | Dahili | Yok |
| **Sekmeler (Tabs)** | Dahili | Yok | Dahili | Dahili | Dahili | Yok |
| **Komut Paleti** | Dahili GPU | Yok | Yok | Dahili | Dahili | Yok |
| **Görsel Protokolleri** | Sixel (GPU Texture) | Yok | Kitty Graphics | Kitty + Sixel + iTerm | Kitty Graphics | Sixel |
| **Klavye Protokolü** | Kitty (Flag 1, 2, 8) | Standart | Kitty (Tam) | Kitty (Geniş) | Kitty (Tam) | Standart |
| **HiDPI Ölçekleme** | Var | Var | Var | Var | Var | Var |
| **Zero-Copy Satır Erişimi** | Var (`&[Cell]`) | Var | Var | Var | Var | Var |
| **Kernel Geri Basınç (Backpressure)** | Var (`sync_channel`) | Kısmi | Var | Var | Var | Var |

---

## 4. Gerçekleştirilen Mimari Düzeltmeler

1. **`get_display_line` Sıfır Kopyalama**:
   - `Grid::get_display_line` fonksiyonu her karede satır satır `Vec<Cell>` allocate edip klonlamak yerine doğrudan `&[Cell]` dilimi döndürür.
   - 60 FPS'te saniyede gerçekleşen binlerce heap tahsisi sıfırlanmıştır.

2. **OSC 8 Deterministik FIFO Tahliyesi**:
   - Standart `HashMap` rastgele tahliyesi yerine `VecDeque` destekli deterministik FIFO kuyruğuna geçilmiştir.
   - Limit aşıldığında aktif ekrandaki yeni linklerin rastgele silinme kusuru giderilmiştir.

3. **Kitty Klavye Protokolü Tuş Bırakma (Release) Desteği**:
   - `ElementState::Released` filtresi kaldırılarak Kitty Flag 2 kapsamında `:3` (release) ve `:2` (repeat) olay kodlaması eklenmiştir.

4. **HiDPI ve Ölçekleme Entegrasyonu**:
   - `WindowEvent::ScaleFactorChanged` dinlenerek pencere, renderer ve font metrikleri ölçek faktörüne göre dinamik ölçeklenir.
