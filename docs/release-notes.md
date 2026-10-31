ALOL 0.5.3 добавляет поддержку Fedora 44+ x86_64: нативный RPM,
установщик для dnf и автоматическую сборку/проверку в Fedora через GitHub Actions.
Симметричный интерфейс и скриншоты из отдельного демонстрационного профиля сохранены.

<details>
<summary><strong>Посмотреть скриншоты</strong></summary>

Снимки сделаны с демонстрационным профилем; каталог модов содержит примеры.

![Главный экран](https://raw.githubusercontent.com/Velmi1337/ALOL/v0.5.3/docs/screenshots/main.png)
![Настройки](https://raw.githubusercontent.com/Velmi1337/ALOL/v0.5.3/docs/screenshots/settings.png)
![Моды](https://raw.githubusercontent.com/Velmi1337/ALOL/v0.5.3/docs/screenshots/mods.png)

</details>

ALOL — нативный лаунчер Arizona RP на Rust для Linux.

Для Arch Linux и CachyOS x86_64 скачайте `.pkg.tar.zst` и установите:

```sh
sudo pacman -U ./alol-0.5.3-1-x86_64.pkg.tar.zst
alol
```

Для Fedora 44+ x86_64 после завершения workflow **Fedora RPM** появится RPM:

```sh
sudo dnf install ./alol-0.5.3-1.fc44.x86_64.rpm
alol
```

Fedora RPM устанавливает Wine, winetricks и русскую локаль. В новой конфигурации
ALOL выбирает доступный UMU/Proton либо системный Wine. Существующие настройки
окружения сохраняются. Проверка пакета не заменяет проверку игры на сервере.

Для Arch/CachyOS нужны включённый multilib и подходящие 32/64-битные драйверы.
При первой подготовке окружения нужен интернет для Windows-компонентов и Proton.

Сборка из исходников:

```sh
git clone https://github.com/Velmi1337/ALOL.git
cd ALOL
./install-arch.sh     # Arch / CachyOS
./install-fedora.sh   # Fedora
```

`SHA256SUMS` относится к исходникам, Arch-пакету, рецептам и скриншотам.
`SHA256SUMS.fedora` содержит проверенные контрольные суммы Fedora-пакета и исходников.
RPM собирается и устанавливается в чистой Fedora 44 перед загрузкой в релиз.
В релиз не входят профили, префиксы, игровые файлы, логи, токены и домашние пути.
Автор пакетов — `ALOL contributors`, хост RPM-сборки — `alol-builder`.

Проект не связан с Arizona Games. Пароль аккаунта вводится в игре.
