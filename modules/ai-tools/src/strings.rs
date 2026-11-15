//! Runtime strings shown in the app (views and approval prompts), in the
//! twelve app locales. Text sent to AI tools stays English.

pub(crate) const LOCALES: [&str; 12] = [
    "en", "de", "es", "fr", "it", "ja", "ko", "pl", "pt", "tr", "uk", "zh",
];
const TEXT_COUNT: usize = 33;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Text {
    ConnectionsTitle,
    NoConnections,
    Owner,
    LastUsed,
    NeverUsed,
    Revoked,
    ReadHistory,
    ReadSnippets,
    WriteClipboard,
    Organize,
    ViewActivity,
    AllowChanges,
    ReadOnly,
    Revoke,
    Remove,
    ActivityTitle,
    ActivityEmpty,
    ActivityEntry,
    Connect,
    SetupHint,
    RestartNote,
    PolicyNote,
    ServedTitle,
    ServedMessage,
    ApprovalTitle,
    ApprovalNote,
    AllowOnce,
    AlwaysAllow,
    Allow,
    Deny,
    VolumeTitle,
    VolumeMessage,
    UnknownDevice,
}

/// The string for `locale` (`uk`, `pt-BR`, `zh_Hans`), English otherwise.
pub(crate) fn text(locale: &str, text: Text) -> &'static str {
    let language = locale
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let column = LOCALES
        .iter()
        .position(|candidate| *candidate == language)
        .unwrap_or(0);
    TABLE[text as usize][column]
}

/// `template` with each `{name}` replaced.
pub(crate) fn fill(template: &str, values: &[(&str, &str)]) -> String {
    values
        .iter()
        .fold(template.to_owned(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), value)
        })
}

const TABLE: [[&str; LOCALES.len()]; TEXT_COUNT] = [
    // ConnectionsTitle
    [
        "Connected tools",
        "Verbundene Tools",
        "Herramientas conectadas",
        "Outils connectés",
        "Strumenti collegati",
        "接続済みのツール",
        "연결된 도구",
        "Połączone narzędzia",
        "Ferramentas conectadas",
        "Bağlı araçlar",
        "Підключені інструменти",
        "已连接的工具",
    ],
    // NoConnections
    [
        "No AI tools connected yet",
        "Noch keine KI-Tools verbunden",
        "Aún no hay herramientas de IA conectadas",
        "Aucun outil d’IA connecté",
        "Nessuno strumento IA collegato",
        "接続された AI ツールはまだありません",
        "아직 연결된 AI 도구가 없습니다",
        "Brak połączonych narzędzi AI",
        "Nenhuma ferramenta de IA conectada",
        "Henüz bağlı yapay zekâ aracı yok",
        "Ще немає підключених інструментів ШІ",
        "尚未连接 AI 工具",
    ],
    // Owner
    [
        "On {device}",
        "Auf {device}",
        "En {device}",
        "Sur {device}",
        "Su {device}",
        "{device} 上",
        "{device}에서",
        "Na: {device}",
        "Em {device}",
        "Cihaz: {device}",
        "На {device}",
        "位于 {device}",
    ],
    // LastUsed
    [
        "Connected · last used {date}",
        "Verbunden · zuletzt genutzt {date}",
        "Conectada · último uso {date}",
        "Connecté · dernière utilisation le {date}",
        "Collegato · ultimo uso {date}",
        "接続済み · 最終使用 {date}",
        "연결됨 · 마지막 사용 {date}",
        "Połączone · ostatnio użyte {date}",
        "Conectada · último uso {date}",
        "Bağlı · son kullanım {date}",
        "Підключено · востаннє використано {date}",
        "已连接 · 上次使用 {date}",
    ],
    // NeverUsed
    [
        "Waiting for first connection",
        "Wartet auf erste Verbindung",
        "Esperando la primera conexión",
        "En attente de la première connexion",
        "In attesa della prima connessione",
        "最初の接続を待っています",
        "첫 연결 대기 중",
        "Oczekuje na pierwsze połączenie",
        "Aguardando a primeira conexão",
        "İlk bağlantı bekleniyor",
        "Очікує першого підключення",
        "等待首次连接",
    ],
    // Revoked
    [
        "Revoked",
        "Widerrufen",
        "Revocada",
        "Révoqué",
        "Revocato",
        "取り消し済み",
        "취소됨",
        "Odwołane",
        "Revogada",
        "İptal edildi",
        "Відкликано",
        "已撤销",
    ],
    // ReadHistory
    [
        "Read history",
        "Verlauf lesen",
        "Leer historial",
        "Lire l’historique",
        "Leggere la cronologia",
        "履歴の読み取り",
        "기록 읽기",
        "Odczyt historii",
        "Ler o histórico",
        "Geçmişi okuma",
        "Читати історію",
        "读取历史",
    ],
    // ReadSnippets
    [
        "Read snippets",
        "Textbausteine lesen",
        "Leer fragmentos",
        "Lire les extraits",
        "Leggere i frammenti",
        "スニペットの読み取り",
        "스니펫 읽기",
        "Odczyt fragmentów",
        "Ler trechos",
        "Parçacıkları okuma",
        "Читати фрагменти",
        "读取片段",
    ],
    // WriteClipboard
    [
        "Write clipboard",
        "In die Zwischenablage schreiben",
        "Escribir en el portapapeles",
        "Écrire dans le presse-papiers",
        "Scrivere negli appunti",
        "クリップボードへの書き込み",
        "클립보드에 쓰기",
        "Zapis do schowka",
        "Gravar na área de transferência",
        "Panoya yazma",
        "Записувати в буфер обміну",
        "写入剪贴板",
    ],
    // Organize
    [
        "Organize (pin, save snippets)",
        "Organisieren (anheften, Textbausteine speichern)",
        "Organizar (fijar, guardar fragmentos)",
        "Organiser (épingler, enregistrer des extraits)",
        "Organizzare (fissare, salvare frammenti)",
        "整理（ピン留め、スニペットの保存）",
        "정리 (고정, 스니펫 저장)",
        "Porządkowanie (przypinanie, zapisywanie fragmentów)",
        "Organizar (fixar, salvar trechos)",
        "Düzenleme (sabitleme, parçacık kaydetme)",
        "Упорядковувати (закріплювати, зберігати фрагменти)",
        "整理（置顶、保存片段）",
    ],
    // ViewActivity
    [
        "View activity",
        "Aktivität anzeigen",
        "Ver actividad",
        "Voir l’activité",
        "Mostra attività",
        "アクティビティを表示",
        "활동 보기",
        "Pokaż aktywność",
        "Ver atividade",
        "Etkinliği göster",
        "Переглянути активність",
        "查看活动",
    ],
    // AllowChanges
    [
        "Allow changes",
        "Änderungen erlauben",
        "Permitir cambios",
        "Autoriser les modifications",
        "Consenti modifiche",
        "変更を許可",
        "변경 허용",
        "Zezwól na zmiany",
        "Permitir alterações",
        "Değişikliklere izin ver",
        "Дозволити зміни",
        "允许更改",
    ],
    // ReadOnly
    [
        "Read only",
        "Nur lesen",
        "Solo lectura",
        "Lecture seule",
        "Sola lettura",
        "読み取り専用",
        "읽기 전용",
        "Tylko odczyt",
        "Somente leitura",
        "Salt okunur",
        "Лише читання",
        "只读",
    ],
    // Revoke
    [
        "Revoke",
        "Widerrufen",
        "Revocar",
        "Révoquer",
        "Revoca",
        "取り消す",
        "취소",
        "Odwołaj",
        "Revogar",
        "İptal et",
        "Відкликати",
        "撤销",
    ],
    // Remove
    [
        "Remove",
        "Entfernen",
        "Quitar",
        "Retirer",
        "Rimuovi",
        "削除",
        "제거",
        "Usuń",
        "Remover",
        "Kaldır",
        "Вилучити",
        "移除",
    ],
    // ActivityTitle
    [
        "Activity of {name}",
        "Aktivität von {name}",
        "Actividad de {name}",
        "Activité de {name}",
        "Attività di {name}",
        "{name} のアクティビティ",
        "{name} 활동",
        "Aktywność: {name}",
        "Atividade de {name}",
        "{name} etkinliği",
        "Активність: {name}",
        "{name} 的活动",
    ],
    // ActivityEmpty
    [
        "No activity yet",
        "Noch keine Aktivität",
        "Aún no hay actividad",
        "Aucune activité",
        "Ancora nessuna attività",
        "まだアクティビティはありません",
        "아직 활동이 없습니다",
        "Brak aktywności",
        "Nenhuma atividade ainda",
        "Henüz etkinlik yok",
        "Активності ще немає",
        "暂无活动",
    ],
    // ActivityEntry
    [
        "{tool} · results: {count}",
        "{tool} · Ergebnisse: {count}",
        "{tool} · resultados: {count}",
        "{tool} · résultats : {count}",
        "{tool} · risultati: {count}",
        "{tool} · 結果: {count}",
        "{tool} · 결과: {count}",
        "{tool} · wyniki: {count}",
        "{tool} · resultados: {count}",
        "{tool} · sonuç: {count}",
        "{tool} · результатів: {count}",
        "{tool} · 结果：{count}",
    ],
    // Connect
    [
        "Connect AI tool",
        "KI-Tool verbinden",
        "Conectar herramienta de IA",
        "Connecter un outil d’IA",
        "Collega strumento IA",
        "AI ツールを接続",
        "AI 도구 연결",
        "Połącz narzędzie AI",
        "Conectar ferramenta de IA",
        "Yapay zekâ aracı bağla",
        "Підключити інструмент ШІ",
        "连接 AI 工具",
    ],
    // SetupHint
    [
        "Add this to your AI tool's MCP settings. You approve its first connection in CopyPaste.",
        "Füge dies zu den MCP-Einstellungen deines KI-Tools hinzu. Die erste Verbindung bestätigst du in CopyPaste.",
        "Añade esto a los ajustes MCP de tu herramienta de IA. Aprobarás su primera conexión en CopyPaste.",
        "Ajoutez ceci aux réglages MCP de votre outil d’IA. Vous approuvez sa première connexion dans CopyPaste.",
        "Aggiungi questo alle impostazioni MCP del tuo strumento IA. Approverai la prima connessione in CopyPaste.",
        "これを AI ツールの MCP 設定に追加してください。最初の接続は CopyPaste で承認します。",
        "이 내용을 AI 도구의 MCP 설정에 추가하세요. 첫 연결은 CopyPaste에서 승인합니다.",
        "Dodaj to do ustawień MCP narzędzia AI. Pierwsze połączenie zatwierdzasz w CopyPaste.",
        "Adicione isto às configurações MCP da sua ferramenta de IA. Você aprova a primeira conexão no CopyPaste.",
        "Bunu yapay zekâ aracınızın MCP ayarlarına ekleyin. İlk bağlantıyı CopyPaste'te onaylarsınız.",
        "Додайте це до налаштувань MCP вашого інструмента ШІ. Перше підключення ви схвалите в CopyPaste.",
        "将此内容添加到 AI 工具的 MCP 设置中。首次连接需在 CopyPaste 中批准。",
    ],
    // RestartNote
    [
        "Next: restart your AI tool or open a new session.",
        "Als Nächstes: dein KI-Tool neu starten oder eine neue Sitzung öffnen.",
        "Siguiente paso: reinicia tu herramienta de IA o abre una nueva sesión.",
        "Ensuite : redémarrez votre outil d’IA ou ouvrez une nouvelle session.",
        "Poi: riavvia lo strumento IA o apri una nuova sessione.",
        "次に: AI ツールを再起動するか、新しいセッションを開いてください。",
        "다음 단계: AI 도구를 다시 시작하거나 새 세션을 여세요.",
        "Dalej: uruchom ponownie narzędzie AI lub otwórz nową sesję.",
        "Próximo passo: reinicie sua ferramenta de IA ou abra uma nova sessão.",
        "Sonraki adım: yapay zekâ aracınızı yeniden başlatın veya yeni bir oturum açın.",
        "Далі: перезапустіть інструмент ШІ або відкрийте новий сеанс.",
        "下一步：重启你的 AI 工具或打开新会话。",
    ],
    // PolicyNote
    [
        "Sensitive items and detected credentials are never shared, whatever you allow here.",
        "Vertrauliche Einträge und erkannte Zugangsdaten werden nie geteilt, egal was du hier erlaubst.",
        "Los elementos sensibles y las credenciales detectadas nunca se comparten, permitas lo que permitas aquí.",
        "Les éléments sensibles et les identifiants détectés ne sont jamais partagés, quoi que vous autorisiez ici.",
        "Gli elementi sensibili e le credenziali rilevate non vengono mai condivisi, qualunque cosa tu consenta qui.",
        "ここで何を許可しても、機密項目や検出された認証情報は共有されません。",
        "여기서 무엇을 허용하든 민감한 항목과 감지된 자격 증명은 공유되지 않습니다.",
        "Wpisy poufne i wykryte dane logowania nigdy nie są udostępniane, niezależnie od tego, na co tu zezwolisz.",
        "Itens confidenciais e credenciais detectadas nunca são compartilhados, seja o que for permitido aqui.",
        "Hassas öğeler ve algılanan kimlik bilgileri, burada neye izin verirseniz verin asla paylaşılmaz.",
        "Конфіденційні елементи та виявлені облікові дані ніколи не передаються, хоч би що ви тут дозволили.",
        "无论你在这里允许什么，敏感项目和检测到的凭据都不会被分享。",
    ],
    // ServedTitle
    [
        "AI tools connect through your computer",
        "KI-Tools verbinden sich über deinen Computer",
        "Las herramientas de IA se conectan a través de tu equipo",
        "Les outils d’IA se connectent via votre ordinateur",
        "Gli strumenti IA si collegano tramite il tuo computer",
        "AI ツールはコンピュータ経由で接続します",
        "AI 도구는 컴퓨터를 통해 연결됩니다",
        "Narzędzia AI łączą się przez Twój komputer",
        "As ferramentas de IA se conectam pelo seu computador",
        "Yapay zekâ araçları bilgisayarınız üzerinden bağlanır",
        "Інструменти ШІ підключаються через комп’ютер",
        "AI 工具通过你的电脑连接",
    ],
    // ServedMessage
    [
        "Turn on AI tools in CopyPaste on a Mac, Windows or Linux computer, and your phone's history will be available there.",
        "Aktiviere KI-Tools in CopyPaste auf einem Mac-, Windows- oder Linux-Computer, dann ist der Verlauf deines Telefons dort verfügbar.",
        "Activa las herramientas de IA en CopyPaste en un equipo Mac, Windows o Linux y el historial de tu teléfono estará disponible allí.",
        "Activez les outils d’IA dans CopyPaste sur un ordinateur Mac, Windows ou Linux, et l’historique de votre téléphone y sera disponible.",
        "Attiva gli strumenti IA in CopyPaste su un computer Mac, Windows o Linux e la cronologia del telefono sarà disponibile lì.",
        "Mac、Windows、Linux のコンピュータで CopyPaste の AI ツールをオンにすると、スマートフォンの履歴をそこで利用できます。",
        "Mac, Windows 또는 Linux 컴퓨터의 CopyPaste에서 AI 도구를 켜면 휴대전화 기록을 그곳에서 사용할 수 있습니다.",
        "Włącz narzędzia AI w CopyPaste na komputerze z systemem macOS, Windows lub Linux, a historia telefonu będzie tam dostępna.",
        "Ative as ferramentas de IA no CopyPaste em um computador Mac, Windows ou Linux, e o histórico do seu telefone ficará disponível lá.",
        "Bir Mac, Windows veya Linux bilgisayardaki CopyPaste'te yapay zekâ araçlarını açın; telefonunuzun geçmişi orada kullanılabilir olur.",
        "Увімкніть інструменти ШІ в CopyPaste на комп’ютері з Mac, Windows або Linux, і там стане доступною історія вашого телефона.",
        "在 Mac、Windows 或 Linux 电脑上的 CopyPaste 中开启 AI 工具，你手机的历史即可在那里使用。",
    ],
    // ApprovalTitle
    [
        "{client} wants to access CopyPaste",
        "{client} möchte auf CopyPaste zugreifen",
        "{client} quiere acceder a CopyPaste",
        "{client} souhaite accéder à CopyPaste",
        "{client} vuole accedere a CopyPaste",
        "{client} が CopyPaste へのアクセスを求めています",
        "{client}이(가) CopyPaste 접근을 요청합니다",
        "{client} prosi o dostęp do CopyPaste",
        "{client} quer acessar o CopyPaste",
        "{client}, CopyPaste'e erişmek istiyor",
        "{client} хоче отримати доступ до CopyPaste",
        "{client} 请求访问 CopyPaste",
    ],
    // ApprovalNote
    [
        "Allow only tools you started yourself. Without an answer, the request is denied in 2 minutes.",
        "Erlaube nur Tools, die du selbst gestartet hast. Ohne Antwort wird die Anfrage nach 2 Minuten abgelehnt.",
        "Permite solo herramientas que hayas iniciado tú. Sin respuesta, la solicitud se rechaza en 2 minutos.",
        "N’autorisez que des outils que vous avez lancés vous-même. Sans réponse, la demande est refusée au bout de 2 minutes.",
        "Consenti solo strumenti che hai avviato tu. Senza risposta, la richiesta viene negata dopo 2 minuti.",
        "自分で起動したツールだけを許可してください。応答がない場合、リクエストは 2 分後に拒否されます。",
        "직접 실행한 도구만 허용하세요. 응답이 없으면 요청은 2분 후에 거부됩니다.",
        "Zezwalaj tylko narzędziom uruchomionym przez Ciebie. Bez odpowiedzi żądanie zostanie odrzucone po 2 minutach.",
        "Permita apenas ferramentas que você mesmo abriu. Sem resposta, a solicitação é negada em 2 minutos.",
        "Yalnızca kendi başlattığınız araçlara izin verin. Yanıt verilmezse istek 2 dakika içinde reddedilir.",
        "Дозволяйте лише інструменти, які ви запустили самі. Без відповіді запит буде відхилено через 2 хвилини.",
        "只允许你自己启动的工具。如果不回应，请求将在 2 分钟后被拒绝。",
    ],
    // AllowOnce
    [
        "Allow once",
        "Einmal erlauben",
        "Permitir una vez",
        "Autoriser une fois",
        "Consenti una volta",
        "今回のみ許可",
        "이번만 허용",
        "Zezwól raz",
        "Permitir uma vez",
        "Bir kez izin ver",
        "Дозволити один раз",
        "仅允许一次",
    ],
    // AlwaysAllow
    [
        "Always allow",
        "Immer erlauben",
        "Permitir siempre",
        "Toujours autoriser",
        "Consenti sempre",
        "常に許可",
        "항상 허용",
        "Zawsze zezwalaj",
        "Sempre permitir",
        "Her zaman izin ver",
        "Завжди дозволяти",
        "始终允许",
    ],
    // Allow
    [
        "Allow",
        "Erlauben",
        "Permitir",
        "Autoriser",
        "Consenti",
        "許可",
        "허용",
        "Zezwól",
        "Permitir",
        "İzin ver",
        "Дозволити",
        "允许",
    ],
    // Deny
    [
        "Deny",
        "Ablehnen",
        "Denegar",
        "Refuser",
        "Nega",
        "拒否",
        "거부",
        "Odrzuć",
        "Negar",
        "Reddet",
        "Відхилити",
        "拒绝",
    ],
    // VolumeTitle
    [
        "Allow more reading?",
        "Weiteres Lesen erlauben?",
        "¿Permitir seguir leyendo?",
        "Autoriser davantage de lecture ?",
        "Consentire altre letture?",
        "さらに読み取りを許可しますか？",
        "더 읽도록 허용할까요?",
        "Zezwolić na dalszy odczyt?",
        "Permitir mais leitura?",
        "Daha fazla okumaya izin verilsin mi?",
        "Дозволити читати далі?",
        "允许继续读取？",
    ],
    // VolumeMessage
    [
        "{client} has read a lot of clipboard history in this session. Allow more?",
        "{client} hat in dieser Sitzung viel Zwischenablage-Verlauf gelesen. Mehr erlauben?",
        "{client} ha leído mucho historial del portapapeles en esta sesión. ¿Permitir más?",
        "{client} a lu beaucoup d’historique du presse-papiers pendant cette session. En autoriser davantage ?",
        "{client} ha letto molta cronologia degli appunti in questa sessione. Consentire di più?",
        "{client} はこのセッションで多くのクリップボード履歴を読み取りました。さらに許可しますか？",
        "{client}이(가) 이 세션에서 클립보드 기록을 많이 읽었습니다. 더 허용할까요?",
        "Narzędzie {client} odczytało w tej sesji dużo historii schowka. Zezwolić na więcej?",
        "{client} leu muito histórico da área de transferência nesta sessão. Permitir mais?",
        "{client} bu oturumda çok fazla pano geçmişi okudu. Daha fazlasına izin verilsin mi?",
        "Інструмент {client} прочитав багато історії буфера обміну в цьому сеансі. Дозволити більше?",
        "{client} 在本次会话中读取了大量剪贴板历史。允许更多吗？",
    ],
    // UnknownDevice
    [
        "another computer",
        "ein anderer Computer",
        "otro equipo",
        "un autre ordinateur",
        "un altro computer",
        "別のコンピュータ",
        "다른 컴퓨터",
        "inny komputer",
        "outro computador",
        "başka bir bilgisayar",
        "інший комп’ютер",
        "另一台电脑",
    ],
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_locale_resolves_with_an_english_fallback() {
        assert_eq!(text("uk-UA", Text::Revoke), "Відкликати");
        assert_eq!(text("pt_BR", Text::Revoke), "Revogar");
        assert_eq!(text("xx", Text::Revoke), "Revoke");
        assert_eq!(
            fill(text("en", Text::Owner), &[("device", "Mac")]),
            "On Mac"
        );
        for row in TABLE {
            assert!(row.iter().all(|value| !value.trim().is_empty()));
        }
    }
}
