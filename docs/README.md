# Dokumentasi

Indeks semua dokumen. Baca **ARCHITECTURE.md** lebih dulu sebelum mengubah kode.

| Dokumen | Isi |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Struktur modul, arah dependensi, cara menambah shell/compositor |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Toolchain, dependency, dan pemeriksaan environment |
| [FEATURES.md](FEATURES.md) | Fitur yang ada, yang direncanakan, syarat "selesai" |
| [SAFETY.md](SAFETY.md) | Jaminan backup, rollback, dan hal yang tidak boleh dilanggar |
| [SWITCH-FLOW.md](SWITCH-FLOW.md) | Urutan langkah switch Noctalia ⇄ DMS, file yang disentuh tiap langkah |
| [UI.md](UI.md) | Spesifikasi layout dan angka tetap tampilan |


## Prinsip

Dokumen ini adalah spesifikasi, bukan catatan. Kalau kode dan dokumen
berbeda, itu bug: entah kodenya yang salah atau dokumennya yang perlu
diperbarui — putuskan, lalu buat keduanya cocok.

Tiga pertanyaan sebelum menambah kode:

1. Modul mana yang boleh berubah? (aturan di ARCHITECTURE.md)
2. Syarat selesai-nya apa? (FEATURES.md)
3. Apakah ini bisa merusak konfigurasi user? (SAFETY.md)

Sebelum mulai coding, pastikan environment-nya benar:

```bash
./scripts/check-deps.sh
```

## Status project

  Ready to use. UI + backend switch, backup, rollback, dan registri shell sudah berjalan.


Urutan pengerjaan ada di FEATURES.md bagian "Rencana".