# File Chameleon: Tasarım ve Uygulama Planı

## Context (Neden)
Kullanıcı üç açık kaynak (MIT) uygulamanın birleşimi olan, **kendi özgün kodumuzla** yazılmış bir masaüstü uygulaması istiyor:
- **Filee** → internetsiz dosya dönüştürücü; Ctrl ile sürükleyince imleçte açılan format çarkı.
- **Nashi** → linkten video, reels, GIF ve resim indirici (yt-dlp) ile basit medya araçları (sıkıştır, kes, sessize al).
- **Clipboardline** → pano fikri. Kullanıcı bunun **hafif** halini seçti: "akıllı pano".

Hedefler:
- Apple (macOS 26/27) tarzı arayüz.
- Az RAM ve küçük dosya boyutu.
- İnternet sadece medya indirirken ve bir kerelik motor indirmede kullanılacak.
- Bitince GitHub'a yüklenecek (MIT).

Klasör şu an boş. Bilgisayarda kurulu olanlar: Rust 1.99, Node 26, ffmpeg 9 (PATH'te), git. **gh ve yt-dlp kurulu değil.**

## Kullanıcının verdiği kararlar
| Konu | Karar |
|---|---|
| Pano | Akıllı pano. Media linki kopyalanınca "İndir?" önerisi gelir. Resim kopyalanınca "Kaydet/Çevir" önerisi gelir. Son kopyalananlar rafında arama ve sabitleme olur. |
| Motorlar | İlk ihtiyaçta bir kere indirilir. PATH'teki ffmpeg varsa o kullanılır. Kurulum yaklaşık 10 MB olur. |
| Giriş yolu | **Ctrl+sürükle çark menüsü.** Pencereye sürükle-bırak da temel olarak her zaman olacak. Sağ tık menüsü ve global kısayol v1'de yok. |
| Tasarım | Figma okunmayacak. Apple HIG bilgisiyle Windows 11 Mica birleştirilecek. |

## Benim kararlarım (kullanıcı itiraz etmedi)
- **Teknoloji:** Tauri 2, Rust çekirdek, Svelte 5, Vite ve TypeScript.
  - Alternatif .NET Avalonia (Filee'nin yolu) kurulumu yaklaşık 60–80 MB yapar.
  - Alternatif Electron 150 MB'tan büyüktür ve RAM'i ağır kullanır. Bu, "hafif olsun" hedefine ters.
- **Platform:** Önce Windows. Çekirdek (`fc-core`) Tauri'den bağımsız, macOS'a taşınabilir.
- **Dil:** TR ve EN.
- **Ofis belgeleri (LibreOffice):** v1'de yok. Motor yöneticisi ileride isteğe bağlı motor ekleyebilecek yapıda.
- **gallery-dl:** v1'de yok, çünkü GPL lisanslı ve büyük. Instagram fotoğraf gönderileri için `og:image` ile en iyi çaba gösterilir. Olmazsa "desteklenmiyor" mesajı çıkar.
- **Pencere kontrolleri:** Pencerede macOS tarzı trafik ışıkları olur.
- **Yazı tipi ve ikonlar:** Gömülü Inter yazı tipi (OFL, Türkçe glifli) ve Lucide ikonları (ISC). SF Symbols ve SF Pro lisans gereği kullanılmaz.

## Mimari
```
Cargo.toml (workspace; release: opt-level="s", lto="fat", codegen-units=1, strip, panic="abort")
engines.manifest.json          # motor id, sürüm, url, sha256, lisans
crates/fc-core/src/            # saf mantık, Tauri yok, hızlı test
  formats/{registry,planner}.rs    # format grafiği, en fazla 2 adımlı rota
  jobs/{queue,naming,progress}.rs  # motor başına eşzamanlılık, iptal, .partial → rename
  engines/{image,svg,ffmpeg,pdf,data}.rs   # trait Engine (ileride libreoffice.rs)
  engine_mgr/{manifest,download,verify}.rs
  download/{classify,ytdlp,direct}.rs
  clip/{classify,history}.rs       # JSON indeks + diskte PNG, saklama süresi, sabitleme
  proc.rs                          # CREATE_NO_WINDOW + Job Object (yetim süreç kalmaz)
src-tauri/src/{lib,ipc,state,tray,windows,settings}.rs
src-tauri/src/platform/windows/{hook,clipboard,webview,dwm}.rs
src/ index.html + overlay.html (ayrı, çok küçük bundle)
src/app/views/{convert,download,clipboard,recent,settings,tools}
src/lib/{ipc,i18n,stores,ui,wheel}   src/locales/{tr,en}.json
```

**Crate'ler** (boyut için minimum özellikle seçilir):
- Tauri eklentileri: single-instance, autostart, dialog, opener.
- `windows`, `clipboard-win`.
- `image` (sadece png, jpeg, gif, bmp, tiff, ico, webp) ve kayıplı WebP için `webp`.
- `resvg`, `lopdf`, `pdfium-render` (pdfium.dll isteğe bağlı indirilir).
- `pulldown-cmark`, `serde_json`, `serde_yaml`, `toml`, `csv`.
- `ureq` (native-tls), `sha2`, `zip`.
- IPC tip sözleşmesi için `ts-rs`.

**RAM stratejisi:**
- Arka plan işleri tamamen Rust'ta çalışır: tepsi, fare kancası, pano dinleyici ve iş kuyruğu.
- Ana pencere kapatılınca webview yok edilir ve açılınca yeniden oluşturulur.
- Hem çark hem de Apple tarzı bildirimler için önceden hazırlanmış tek bir küçük "overlay" webview kullanılır. Bu webview gizliyken `TrySuspend` ile uyutulur.
- WebView2 düşük kaynak argümanları verilir ve resimler JS belleğinde tutulmaz.
- **Hedefler:** exe 8 MB'tan küçük. Boşta (tepside) toplam özel bellek 150 MB'tan az, ideali yaklaşık 100 MB.

## Riskli parçalar ve çözümleri
1. **Çark (Ctrl+sürükle):**
   - `WH_MOUSE_LL` kancası ayrı bir iş parçacığında çalışır ve her zaman `CallNextHookEx` çağırır.
   - Ctrl basılı + sol tık + `SM_CXDRAG` eşiğinden fazla hareket olunca çark gösterilir. Varsayılan olarak sadece Explorer ve masaüstü kaynaklı sürüklemelerde çalışır (ayarla tüm uygulamalar açılabilir).
   - Overlay, imleç konumunda `SWP_NOACTIVATE` ile gösterilir. Şeffaf, çerçevesiz, her zaman üstte ve odak çalmaz.
   - Tauri'nin drag-drop olayları kullanılır: `Enter` dosya yollarını, `Over` konumu, `Drop` bırakılan dilimi verir.
   - Yaklaşık 500 ms içinde dosya gelmezse çark gizlenir. Böylece metin sürüklemelerinde çıkmaz.
   - Çarkta en fazla 8 dilim olur: en son kullanılan 7 hedef ve "Diğer…".
   - DPI için fiziksel koordinatlar kullanılır.
   - **Yedek plan:** Tauri olayları gelmezse ham pencerede yerel `IDropTarget` kullanılır.
2. **Pano:**
   - `clipboard-win` Monitor kullanılır. Parola yöneticisi işaretleri (`ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory=0`, `CanUploadToCloudClipboard=0`) okunur ve öyle işaretli içerikler atlanır.
   - Kendi kopyaladıklarımız yok sayılır, aynı içerik tekrar kaydedilmez.
   - Resimler 25 MP ile sınırlıdır.
   - Kopyalama anında ağa çıkılmaz. URL, alan adı listesi ve uzantıya göre sınıflandırılır.
3. **YouTube JS gereksinimi:**
   - Sırasıyla PATH'te Node 20 veya üstü varsa `--js-runtimes node:<path>` kullanılır, yoksa ilk YouTube linkinde deno (~35 MB) indirilir, QuickJS ise "lite" seçenek olarak denenir.
   - yt-dlp her zaman `--ignore-config --no-update --no-playlist --newline --ffmpeg-location` ile çalıştırılır.
   - yt-dlp en fazla günde bir, sadece indirme öncesinde güncellenir. SHA, release'teki `SHA2-256SUMS` dosyasıyla doğrulanır.
   - Tarayıcı çerezi (Instagram için) **varsayılan kapalı**, isteğe bağlı bir ayardır.

## v1 format matrisi
- **Resim:**
  - png, jpg, webp (kayıplı ve kayıpsız), bmp, gif, tiff ve ico arasında her yöne dönüşüm. ico çok boyutlu üretilir.
  - svg'den png, jpg, webp ve ico'ya dönüşüm.
  - Seçenekler: kalite ve yeniden boyutlandırma.
  - HEIC/AVIF v1.1'e kalır.
- **Video (ffmpeg):**
  - mp4, mkv, webm, mov ve avi arasında dönüşüm.
  - Videodan GIF (palettegen), GIF'ten mp4 veya webm.
  - Videodan ses çıkarma.
- **Ses:** mp3, m4a, opus/ogg, flac ve wav arasında dönüşüm.
- **Araçlar paneli:** sıkıştır (CRF veya hedef boyut), kes (trim), sessize al, hız ve kırp.
- **PDF:**
  - Birleştir ve böl.
  - Resimlerden PDF.
  - PDF'ten png veya jpg (pdfium isteğe bağlı indirilir).
- **Veri:** md'den html'e, txt'den html'e. csv, json, yaml ve toml kendi aralarında dönüşür.
- **Kural:** Karışık dosyalar bırakılırsa sadece hepsine uygun hedefler gösterilir. Kaynak dosyalar asla silinmez veya üzerine yazılmaz. Aynı ad varsa sonuna " (1)" eklenir.

## Arayüz (Apple tarzı)
- **Ana pencere:** Mica arka plan, yarı saydam kenar çubuğu ve trafik ışıkları. Bölümler: **Dönüştür · İndir · Pano · Son İşlemler · Ayarlar**.
- **Dönüştür:** Büyük bir bırakma kartı, dosya çipleri, uygun formatların ızgarası, seçenekler ve ilerleme kuyruğu.
- **İndir:**
  - URL alanı ve Yapıştır düğmesi.
  - Önizleme kartı: başlık, site, süre, çözünürlük, boyut.
  - Kip seçimi (Video / Ses / GIF / Orijinal) ve kalite seçimi (En iyi / 1080p / 720p).
  - Kayıt klasörü, ilerleme ve iptal (iptalde yarım dosyalar silinir).
  - Sonuçta "Klasörde göster" ve "Çevir" seçenekleri.
- **Pano:** Son N öğe (metin, link, resim), arama, sabitleme, duraklatma ve saklama süresi.
- **Ayarlar:**
  - Genel: Windows ile başla, dil, tema, vurgu rengi.
  - Motorlar: durum, indir veya kaldır, yt-dlp güncelle.
  - Çark: aç/kapat, hangi uygulamalarda çalışacağı.
  - Pano.
  - Çıktı klasörü.
  - Hakkında.
- **Overlay:** Cam görünümlü dairesel çark ve sağ altta Apple tarzı bildirim kartları. Overlay'de `backdrop-filter` kullanılmaz; yarı saydam dolgu, ince kenarlık ve gölge kullanılır.
- **Tepsi menüsü:** Aç, Panoyu göster, Duraklat, Çıkış.

## Uygulama aşamaları
Opus planı yönetir ve her aşamayı inceler. Sonnet alt ajanlar, dosyaları çakışmayan paralel işleri yapar.

0. **Hazırlık:**
   - `git init`.
   - Tasarım belgesi `docs/superpowers/specs/2026-11-19-file-chameleon-design.md` dosyasına yazılır.
   - İskelet kurulur: workspace, tauri.conf, boş modüller, IPC tipleri, manifest şeması.
1. **Kapı denemeleri (spike):**
   - (a) Sürükleme ortasında gösterilen overlay Enter, Over ve Drop olaylarını alıyor ve odak çalmıyor mu?
   - (b) Kanca durum makinesi doğru çalışıyor mu?
   - (c) yt-dlp YouTube'da node, deno ve QuickJS ile çalışıyor mu?
   - (d) Overlay uyutulduğunda RAM ölçümü.
   - Başarısız olan olursa yedek plana geçilir.
2. **Paralel A–E:**
   - A. formats, jobs ve testleri.
   - B. image, svg ve data motorları.
   - C. engine_mgr, ffmpeg motoru ve proc.
   - D. Tasarım sistemi ve kabuk (Mica, trafik ışıkları, Inter, ikonlar, i18n).
   - E. Windows hook, webview ve overlay pencere yöneticisi.
3. **Paralel F–I:**
   - F. download modülü ve İndir ekranı.
   - G. clip modülü, Windows pano dinleyici ve Pano ekranı.
   - H. Çark ve bildirim arayüzü (overlay).
   - I. Dönüştür, Son İşlemler ve Ayarlar ekranları.
4. **Entegrasyon:**
   - Kuyruk arayüze, çark planlayıcıya ve son işlemlere bağlanır.
   - Akıllı pano bildirimleri bağlanır.
   - Araçlar paneli ve PDF motoru eklenir.
5. **Cila:**
   - TR ve EN çevirileri tamamlanır.
   - RAM ve boyut ölçümü yapılıp optimize edilir.
   - Klavye erişimi eklenir.
   - İlk açılışta motor indirme akışı düzenlenir.
6. **GitHub:**
   - Repo içeriği hazırlanır: README.md, README.tr.md, LICENSE (MIT), THIRD_PARTY_NOTICES (ffmpeg GPL, yt-dlp Unlicense, deno MIT, pdfium, Inter OFL, Lucide ISC, resvg MPL-2 ve diğerleri).
   - "Filee, Nashi ve Clipboardline'dan ilham alındı, bağlantılı değil" notu eklenir.
   - İçerik hakları kullanıcının sorumluluğundadır uyarısı eklenir.
   - `ci.yml` her push'ta fmt, clippy, test, svelte-check ve vitest çalıştırır.
   - `release.yml` `v*` etiketiyle tauri-action kullanarak NSIS kurulumunu ve .sha256 dosyasını taslak release'e koyar.
   - **gh kurulumu (winget) ve repo oluşturma/push kullanıcıya sorulup onayı alınarak yapılır.**

## Doğrulama
- **Rust birim testleri (`cargo test --workspace`):**
  - Format kayıt defteri ve rota: sadece geçerli hedefler çıkıyor mu, adım sınırına uyuluyor mu.
  - Dosya adlandırma: çakışma, Windows'un ayrılmış adları, uzun yollar.
  - ffmpeg `-progress` ve yt-dlp ilerleme ve `-J` ayrıştırıcıları (fixture ile).
  - Pano sınıflandırıcısı ve gizlilik işaretleri.
  - SHA doğrulaması, bozuk dosya senaryosu dahil.
- **Entegrasyon testleri:**
  - ffmpeg varsa `lavfi testsrc` ile üretilmiş girdilerle gerçek dönüşüm yapılır.
  - İptal testi: yarım dosya kalmamalı, yetim ffmpeg süreci kalmamalı.
- **Ön yüz:** vitest (çark geometrisi, store'lar) ve svelte-check.
- **Elle uçtan uca test:**
  - Ctrl+sürükle; Explorer'dan, masaüstünden ve tarayıcıdan denenir. Metin sürüklemede çark çıkmamalı.
  - Ölçeklendirme: %125 ve %175 DPI.
  - Esc ile iptal.
  - Parola yöneticisinden kopyalama kaydedilmemeli.
  - İndirme: YouTube, Instagram reels, TikTok, X ve doğrudan GIF/resim linki; iptal de denenir.
  - İnternet kapalıyken dönüştürme çalışmalı.
  - `npm run tauri dev` ile çalıştırılıp ekran görüntüleri alınır.
- **Ölçüm:**
  - RAM: `Get-Process msedgewebview2,file-chameleon | Measure-Object PrivateMemorySize64 -Sum`
  - exe ve kurulum boyutu: `(Get-Item ...).Length`
  - Bağımlılık boyutu: `cargo bloat --release --crates`
