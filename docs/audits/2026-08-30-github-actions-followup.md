# Follow-up: системний аудит GitHub Actions

Дата: 2026-08-30<br>
Репозиторій: `dmytro-yevs/copypaste`<br>
Період: 2026-07-27—2026-08-27 (UTC)<br>
Базовий знімок перевірки: `05ef77584d3fcc9d01e9cda51a8b7521a9dbe57c`

Це уточнення до незмінного [попереднього аудиту](2026-08-30-github-actions.md),
а не його перезапис. Повний машинний перелік 376 спостережень збережено у
[findings.json](2026-08-30-github-actions-findings.json). Кожен рядок прив'язано
до run ID, SHA, URL, джерела діагностики, класифікації, діагностичної
впевненості та явно записаних невідомих. «Висока» впевненість діагностики не є
доказом причинності; змішані корені не розщеплюються і не додаються.

## Висновок

Свіжий інвентар містить 1 764 запуски: 929 успішних, 376 невдалих, 458
скасованих і 1 зі станом `queued` на момент знімка. Інвентар охоплює весь зазначений закритий період;
перевірка join дала 376/376 невдалих run ID без пропусків, дублювань або
зсуву SHA. Водночас причинний розбір не доводить одну причину для кожного
запуску: частина рядків має лише неповну телеметрію, а частина містить кілька
незалежних помилок. Саме тому в JSON збережено спостереження, а не заяву
«усі причини доведені».

Розподіл невдалих запусків (вузькі назви Dependabot і GHAS згруповано для
читабельності; рядок з назвою workflow-шляху входить до Windows):

| Сімейство workflow | Невдачі |
| --- | ---: |
| CI | 102 |
| Android emulator | 74 |
| Browser (WebKitGTK, Linux) | 49 |
| Windows native desktop E2E | 40 |
| E2E (real WebView) | 34 |
| Release | 22 |
| Nightly native matrix | 19 |
| Supply chain | 13 |
| Mutation gate | 8 |
| GitHub Advanced Security | 9 |
| Nightly | 3 |
| Dependabot Updates | 3 |
| **Разом** | **376** |

Дані зібрано шістьма непересічними наборами: CI 96, Browser/E2E 75, Android
68, Windows 34, інші workflow 68 і коригування фактичних тестів за 27 серпня
35. Останній набір не замінює попередні рядки: він додає відсутні run ID та
зберігає 22 випадки з невідомою причиною і 11 рядків із кількома фактичними
помилками.

## Що є системним

### Події, скасування та fan-out

Скасування переважно є штатним supersession для push/PR на тому самому ref,
а не доведеним збоєм. Окремий ризик був у зіткненні manual/schedule/main і в
повторному розгортанні Android sweep через matrix caller. Event-aware групи
скасовують лише застарілий push/PR, а scheduled Android sweep викликається один
раз; відповідні wiring/mutation перевірки вже зелені. Це контроль конфігурації,
не ретроактивна причина всіх 458 скасувань.

### Gate/toolchain drift

У частині CI червоніли локальні контракти: stale `Cargo.lock`, TS/Rust API або
форматування, а також Docker-залежний dependency gate. Перехід на pinned
`cargo-deny` із checksum-перевіркою та прямі locked-перевірки вже інтегровані.
Репрезентативні первинні сигнали — [locked Cargo.lock failure у E2E
30588916123](https://github.com/dmytro-yevs/copypaste/actions/runs/30588916123),
де inventory підтверджує `E2E (real WebView)` і точний `cargo build --locked`
error, та [відсутній Tcl у Nightly
30254035876](https://github.com/dmytro-yevs/copypaste/actions/runs/30254035876).
У поточному [Supply chain run
33280200416](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200416)
gate green. Три історичні scanner runs дали 9, 9 і 3 знахідки — [32004916284](https://github.com/dmytro-yevs/copypaste/actions/runs/32004916284),
[32700921501](https://github.com/dmytro-yevs/copypaste/actions/runs/32700921501),
[32944162074](https://github.com/dmytro-yevs/copypaste/actions/runs/32944162074); вони
належать синтетичним fixture/історичним fingerprint allowlist, а не
підтвердженим live secrets чи validated vulnerability. Не можна послаблювати
сканер.

### Readiness, authoritative state та UI-контракти

Частина падінь перевіряла stale snapshot, skeleton або видимий текст замість
авторитетного стану. Android startup тепер має детерміновану permission
hydration fence, pairing provider і document-scroll geometry мають окремі
контракти, а lazy-route retry відтворює failed import. Проте в свіжому
[Android run 33280200527](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200527)
Devices 4/4 і Done вже проходять; API 33 також green. Локальна корекція
виправила Pin badge/Unpin icon assertion. Наступна перевірка має лише повторити
точний кандидат і прочитати нові modern-IME frame та restart diagnostics;
heading/compact-search/Done/Promise-перевірки не слід робити наново як начебто
невиправлені. Це не підстава додавати delay або збільшувати timeout.

### Захищені Windows-докази

Pairing code має залишатися password semantics (`IsPassword=true`), у
protected root, поза capture та з redaction у помилках. У
[CI run 33280200310](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200310)
залишає source-boundary та protected-UIA спостереження. Це не дозвіл
послабити UIA-вимогу: власник доказу має прочитати нові diagnostics і
відокремити відсутній елемент від помилки продукту.

### Browser/Windows E2E та межі доказу

Поточні [Browser E2E run 33280200330](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200330)
і [Windows E2E run 33280200306](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200306)
мають Done passed, але Pin залишається failed через старі badge/Unpin
спостереження; локальна корекція вже закрила unawaited-Promise дефект. Тепер
потрібні лише rerun exact candidate і читання нової diagnostics для Pin badge,
Unpin icon та toolbar identity. WebKitGTK — спільний UI-шар, не native evidence
для macOS, Android чи Windows.

Широкі старі класифікації «driver crash = infrastructure» відхилено: passing
harness guard навмисно падає, щоб довести guard. Свіжі mixed-cause рядки не
перетворюються на одну root cause без окремого log/test review. Окремий
[історичний Mutation delta 33101729921](https://github.com/dmytro-yevs/copypaste/actions/runs/33101729921)
засвідчує mismatch fixture/self-test, а не регресію продукту; поточний
[Mutation run 33280200297](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200297)
green.

### Довговічні контролі

| Контроль | Статус на знімку | Перевірка прийняття |
| --- | --- | --- |
| Event-aware concurrency і один Android fan-out | **Реалізовано** у workflow/wiring baseline | Матриця викликає scheduled sweep один раз; cancellation не приписується цій причині без run-level доказу. |
| Portable gate config, wiring/mutation contract checks | **Реалізовано**; Supply/Mutation current runs green | Одна конфігурація gate для local/CI; mutation не може пройти через відсутній producer/consumer. |
| Target-OS compile/unit boundary для Windows | **F5 compile пройшов; source-boundary/UIA blocker** | Зберегти exact build evidence і прочитати source-boundary та protected-UIA failures; глобальний `unsafe` allowance не додається. |
| Semantic E2E observations і negative fixtures | **Реалізовано локально; native rerun pending** | Receipts доводять authoritative state, identity і negative guard; WebKit/local proof не замінює native evidence. |
| Failure-artifact preservation і privacy-safe diagnostics | **Контроль потрібний; coverage неповна** | Logs/jobs/artifacts мають зберігатися та не розкривати секрети; evidence loss не класифікується як product/infrastructure. |
| Exact candidate/bytes qualification before release tag | **Обов'язкова release-вимога; evidence неповна** | Один commit/artifact identity, macOS, physical Android і installed Windows receipts; tag/release блокується без них. |
| Branch protection/required checks | **Пропозиція, потребує окремої дії** | Перевірити й налаштувати required checks після user-authorized policy change; у свіжій перевірці rulesets — `[]`. |

## Поточний acceptance snapshot

На SHA `05ef77584…` Supply і Mutation green у [33280200416](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200416)
та [33280200297](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200297);
API 36 shipped smoke (22), storage (18), Cloud (17) та API 33 green у [Android
PR run 33280200527](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200527).
Debug 51/52 залишає frame-parser спостереження. Browser і Windows E2E мають
Done passed, але Pin failed на старих badge/Unpin assertions у [33280200330](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200330)
та [33280200306](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200306).
CI [33280200310](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200310)
залишається 20/22 через source-boundary та protected-UIA спостереження.

API 34 debug-attach observations відсутні у [manual run
33280198553](https://github.com/dmytro-yevs/copypaste/actions/runs/33280198553);
у shipped Cloud visible error відсутній потрібний AX-доказ. Фізичний Android,
macOS і installed Windows release evidence ще не завершені. Plain-wrapper
Cloud native experiment не доведений. Локальні виправлення, навіть reviewed
або held для rerun, не підвищують цей статус до native proof. Усі твердження про
release залишаються «не кваліфіковано», доки receipt не зв'яже commit, run,
платформу, сценарій, accessibility evidence і бюджети.

На локальному correction candidate
`093e90b9d646fdae3621e2dd453e63c9c68054ae` gates дали UI 425, Node 86, E2E 15,
Android 130, smoke 85, wiring 572 і mutation 91/0; Browser/Cloud — 320/390,
із фактичним gap 8 і одним alert без console error. Це корисна перевірка коду та harness,
але не native proof і не підстава оголошувати release готовим.

## Наступні кроки

| Пріоритет / власник | Прийняття |
| --- | --- |
| P0 — Windows native evidence owner | На тому самому commit повторити [CI 33280200310](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200310) і прочитати нові source-boundary/protected-UIA diagnostics: named `IsPassword=true` element, protected root, capture exclusion і redacted failure; відсутність елемента залишається blocker. |
| P0 — Browser/Windows E2E owner | Повторити exact candidate для [Browser 33280200330](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200330) і [Windows 33280200306](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200306); прочитати Pin badge/Unpin icon та toolbar identity diagnostics. Done вже passed, Pin — failed; зелені Android heading/compact-search/Promise перевірки не відкривати як нові fixes; не вилучати assertion і не додавати retry. |
| P0 — Android native evidence owner | Повторити exact candidate з [Android 33280200527](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200527) і прочитати modern-IME frame та local-restart diagnostics; не відтворювати вже зелені Devices 4/4, compact-search, Done або Promise checks як нові fixes. |
| P1 — cloud/release owner | Провести plain-wrapper Cloud native experiment, розібрати visible error/AX та [API34 manual 33280198553](https://github.com/dmytro-yevs/copypaste/actions/runs/33280198553); додати physical Android/macOS/installed Windows receipts і bind exact commit/run/artifact. |
| P1 — supply-chain owner | Зберегти green [Supply 33280200416](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200416) та [Mutation 33280200297](https://github.com/dmytro-yevs/copypaste/actions/runs/33280200297); синтетичні fixture findings не оголошувати live vulnerability. |

Жоден пункт не передбачає збільшення timeout, blind retry, skip/waiver,
`continue-on-error` чи послаблення security/accessibility assertion.

## Обмеження та походження даних

[`31533839104`](https://github.com/dmytro-yevs/copypaste/actions/runs/31533839104) — відомий failure статус, але evidence loss: logs HTTP 404, jobs і
artifacts HTTP 200 з порожнім вмістом, локальний log 0 bytes; причинний висновок
неможливий.
[`32367887300`](https://github.com/dmytro-yevs/copypaste/actions/runs/32367887300) — job log підтверджує `testUniversalDebugUnitTest`: 35 тестів,
7 failed, але JUnit не прикріплено; імена та assertions відсутні, тому рядок
залишається unknown. У багатьох 35 delta-рядках причина навмисно
залишається unknown через mixed failures або неповний log. Це не можна
перекласифіковувати як infrastructure/product лише за назвою workflow.

Наведені 82.862 годин скасованих jobs — тільки старий snapshot із 452
скасуваними; для свіжих 458 тривалість не обчислювалася і це не billable
usage. Root-family counts є counts рядків/спостережень: overlapping multi-root
класи не можна сумувати як unique failures. Попередній audit і tracked
inventory залишаються незмінними; findings JSON містить усі 376 рядків і їхні
невідомі.
