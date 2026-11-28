import type { Dict } from './fr'
// Português do Brasil.
const ptBR: Dict = {
  meta: { title: 'POP · O SaaS dos criadores de UEFN' },
  nav: { features: 'Recursos', pricing: 'Preços', faq: 'FAQ', login: 'Entrar', join: 'Entrar na beta', home: 'POP, início', lang: 'Idioma', search: 'Buscar um país' },
  menu: { groups: ['Organizar', 'Conectar o UEFN', 'Fazer o mapa crescer'], items: [{ t: 'Tarefas e planejamento', d: 'Tarefas de uma hora, marcos e atrasos acompanhados.' }, { t: 'Páginas', d: 'Documentos, regras e bancos de dados do mapa.' }, { t: 'Quadro branco', d: 'Post-its, formas e setas, em equipe ao vivo.' }, { t: 'Equipe e funções', d: 'Convide, dê uma função, cada um sabe o que fazer.' }, { t: 'Organizador', d: 'Os atores do nível organizados e editados em lote.' }, { t: 'UEFN Link', d: 'Seus projetos UEFN sincronizados com o POP.' }, { t: 'IAs conectadas', d: 'Claude, Cursor ou Codex avançam nas suas tarefas.' }, { t: 'POP Forge', d: 'O app de desktop que constrói no UEFN.' }, { t: 'Análise de mercado', d: 'Os gêneros que atraem e os que estão saturados.' }, { t: 'Tendências de vídeo', d: 'O que bomba no TikTok e no YouTube Shorts.' }, { t: 'Playtests e lançamento', d: 'Coloque o mapa em teste e prepare o lançamento.' }, { t: 'Pagamentos', d: 'Divisão de ganhos e extrato de cada um.' }], bottom: ['Discord e GitHub conectados', 'No Mac e no PC'], all: 'Todos os recursos' },
  hero: {
    announce: 'Beta fechado aberto a criadores de UEFN', h1a: 'Crie no UEFN.', h1b: 'Sozinho ou em equipe, sem caos.',
    subA: 'O POP é o SaaS que ajuda criadores de UEFN da ideia ao lançamento: ', subB: 'tarefas de uma hora', subC: ', quadro branco, IAs conectadas, análise de mercado e divisão de ganhos.',
    cta: 'Entrar na lista de espera', see: 'Ver os recursos', micro1: 'Beta fechado', micro2: 'Acesso por lista de espera', micro3: 'Conectado ao UEFN',
  },
  marquee: { label: 'Funciona com as suas ferramentas', epic: 'API do Fortnite da Epic' },
  pain: {
    eyebrow: 'O problema', h2a: 'Discord, Google Docs, arquivos por todo lado…', h2b: 'e o mapa não anda.', without: 'Sem POP', with: 'Com POP',
    bad: ['“Quem mexeu no spawner?”', 'As ideias se perdem no Discord', 'Uma lista de tarefas num Google Doc', 'A planilha de ganhos, versão 7', '“A gente lança quando mesmo?”', 'Construir sem saber se o gênero funciona'],
    good: ['Toda ação é registrada e pode ser desfeita', 'As ideias do quadro viram tarefas', 'Tarefas de uma hora, com dependências', 'Um extrato claro para cada um', 'Marcos e atrasos acompanhados automaticamente', 'Um veredito por gênero, com números reais'],
  },
  feat: {
    eyebrow: 'Recursos', h2: 'Tudo para criar no UEFN, da ideia ao dia do lançamento.', p: 'O que você vê aqui são as telas reais do POP, simplificadas.',
    items: [
      { tag: 'Tarefas e planejamento', h: 'Tarefas de uma hora. Um planejamento que se cumpre.', p: 'O gerador divide o mapa em tarefas de uma hora com suas dependências. O planejamento acompanha os marcos e aponta atrasos, e um assistente integrado resume o dia.', pts: ['Kanban e dependências', 'Marcos e atrasos acompanhados', 'Resumo do dia'] },
      { tag: 'Organizador', h: 'Todos os seus dispositivos. Organizados em três segundos.', p: 'Busque por tag, tipo ou pasta, selecione vinte e renomeie, mova ou duplique de uma vez. Ctrl+Z desfaz. Se o UEFN estiver fechado, as mudanças esperam.', pts: ['Busca tag:, type:, pasta:', 'Edição em lote', 'Tudo pode ser desfeito'] },
      { tag: 'Quadro branco', h: 'Desenhe o mapa em equipe.', p: 'Post-its, formas, setas, cores, negrito, sublinhado: tudo para organizar as ideias. Votem e transformem os melhores post-its em tarefas.', pts: ['Formatação completa', 'Tempo real em equipe', 'Post-its → tarefas'] },
      { tag: 'Análise', h: 'Saiba o que construir antes de construir.', p: 'Quais gêneros atraem jogadores, onde há espaço, quais mapas decolam no lançamento e o que bomba em vídeo. Números da API oficial da Epic.', pts: ['Veredito por gênero', 'Mapas que funcionam', 'Tendências TikTok e Shorts'] },
      { tag: 'Pagamentos', h: 'Cada um sabe quanto recebe.', p: 'Regras de divisão por porcentagem, valor fixo ou por tarefa, importação de ganhos, extrato por pessoa. Chega de planilha perdida no Discord.', pts: ['Porcentagem, fixo, por tarefa', 'Importação CSV', 'Extrato de cada um'] },
    ],
    aiB: 'Já usa uma IA?', aiText: 'Conecte Claude, Cursor, Codex ou outra IA compatível com MCP: ela pega uma tarefa, escreve o Verse, compila e fecha a tarefa com uma nota. Toda ação é registrada e pode ser desfeita. Compatíveis: Claude Code, Claude Desktop, Cursor, Codex, Windsurf, VS Code (GitHub Copilot), Gemini CLI.',
  },
  stats: [['1 h', 'por tarefa, para avançar todo dia'], ['0', 'serviço de IA externo obrigatório'], ['7 dias', 'de estatísticas por mapa, da API da Epic'], ['100%', 'das ações no UEFN podem ser desfeitas']],
  pricing: {
    eyebrow: 'Preços', h2: 'Escolha seu plano.', p: "O POP está em beta fechado: o acesso é pela lista de espera. Na abertura, toda conta começa com 7 dias de teste grátis do Pro, sem cartão.",
    perMonth: '/ mês', compare: 'Comparar planos', unlimited: 'Ilimitado', soon: 'Em breve',
    plans: [
      { name: 'Starter', tag: "Para criar", desc: "Todo o espaço de trabalho para criar seus mapas, sozinho ou em equipe.", cta: 'Entrar na lista' },
      { name: 'Pro', tag: "O mais completo", desc: "Tudo do Starter, mais a análise: ideias de mapas, mercado, acompanhamento do Discover.", cta: 'Entrar na lista' },
    ],
    rows: ['Organização', 'Tarefas de uma hora, Kanban, dependências', 'Assistente integrado (resumo do dia, atrasos)', 'Páginas, quadro branco, planejamento e marcos', 'Mapas', 'Membros', 'UEFN', 'UEFN Link: projetos sincronizados', 'Organizador de atores e edição em lote', 'IAs conectadas (Claude, Cursor, Codex…)', 'POP Forge: construção assistida no UEFN', 'Crescimento', "Ideias de mapas, análise de mercado, acompanhamento do Discover", 'Tendências de vídeo TikTok e YouTube Shorts', 'Troca de playtests e dia do lançamento', 'Divisão de ganhos e extratos', 'Discord, GitHub, Lore'],
    perDay: ["Menos de 0,50 € por dia", "Menos de 1 € por dia"],
    hints: ["", "Seu mapa dividido em tarefas de uma hora, com o que bloqueia o quê", "Toda manhã: o que está atrasado e por onde começar", "A documentação do mapa, ideias ao vivo e o calendário", "Quantos mapas você quiser", "Convide a equipe toda sem custo extra", "", "Suas IAs veem o projeto UEFN real: atores e Verse", "", "Conecte sua IA por MCP; toda ação pode ser desfeita", "Construir no UEFN a partir de um pedido", "", "Os gêneros que atraem jogadores e onde ainda há espaço", "O que está crescendo agora no TikTok e no YouTube Shorts", "Encontre testadores e prepare o dia do lançamento", "Quem recebe quanto, com um extrato para cada um", "Notificações e links para suas ferramentas"],
  },
  faq: {
    eyebrow: 'FAQ', h2: 'As perguntas que nos fazem.',
    items: [
      ['Preciso de um PC com Windows?', 'Para conectar o UEFN, sim: o UEFN só roda no Windows, e o UEFN Link roda nesse PC. O resto do POP funciona em qualquer navegador, inclusive no Mac.'],
      ['O POP publica meu mapa?', 'Não. O POP prepara tudo (verificação antes da publicação, playtests, plano do dia do lançamento), mas a publicação é sempre feita no portal de criadores da Epic.'],
      ['O POP funciona se eu criar sozinho?', "Sim. O POP serve tanto para criadores solo quanto para equipes, desde o plano Starter: você pode convidar membros depois sem mudar nada."],
      ['Quais IAs posso conectar?', 'As que aceitam servidores MCP: Claude Code, Claude Desktop, Cursor, Codex, Windsurf, VS Code (GitHub Copilot), Gemini CLI. O Claude Desktop exige que o POP esteja online com HTTPS. A assinatura da sua IA não está incluída no POP. IAs de chat no navegador (ChatGPT, Gemini, Copilot na web) ainda não são suportadas. Depois de conectada em “IAs conectadas”, sua IA lê e edita suas tarefas e atores do UEFN, escreve Verse, e toda ação é registrada e pode ser desfeita.'],
      ['De onde vêm os números da análise?', 'Da API oficial do Fortnite da Epic: jogadores, pico de jogadores simultâneos, tempo de jogo e retenção de cada mapa público, por 7 dias.'],
      ['Como consigo acesso?', 'O POP está em beta fechado. Entre na lista de espera: analisamos os pedidos um a um e depois enviamos seu usuário e uma senha provisória, que você troca no primeiro acesso.'],
      ['Quanto custa?', 'Os preços estão mais acima. Eles valem no lançamento do POP; os membros do beta são avisados antes de qualquer pagamento.'],
    ],
  },
  final: { h2a: 'Seu próximo mapa', h2b: 'começa aqui.', p: 'Entre na lista de espera: enviamos seu acesso assim que abrir uma vaga.' },
  foot: { legalTitle: 'Legal', made: 'Feito por criadores de Fortnite, para criadores de Fortnite.', legal: 'Fortnite e UEFN são marcas da Epic Games. O POP não é afiliado nem endossado pela Epic Games.', product: 'Produto', help: 'Ajuda', account: 'Conta' },
  mock: {
    overview: 'Visão geral', nav: ['Visão geral', 'Páginas', 'Quadro branco', 'Tarefas', 'Organizador', 'Mercado'],
    hello: 'Oi Léa, sugiro começar pela #42.', late: '2 tarefas atrasadas (a mais antiga era para 12 de nov.).', mile: 'Marco “Beta” em 5 dias: faltam 9 tarefas, atraso previsto de 1 dia.',
    take: 'Pegar', start: 'Começar', unlocks: 'desbloqueia outras 3 tarefas', today: 'prevista para hoje', waits: 'aguarda #42',
    todo: 'A fazer', doing: 'Em andamento', done: 'Feito', tasks: ['Sistema de pontuação', 'Cenário da zona 2', 'Tela final', 'Playtest interno', 'Spawn dos jogadores', 'Tempestade'],
    sent: 'enviado ao UEFN', err: 'Linha 12: int e float não podem ser somados.', fix: 'Corrigir: Points += 1',
    red: 'Time vermelho', selected: '2 selecionados', rename: 'Renomear', move: 'Mover', dup: 'Duplicar',
    s1: 'Modo times 4v4', s2a: 'Chefe final', s2b: 'no centro', s3: 'Economia: 1 moeda / eliminação',
    verdicts: ['Boa oportunidade', 'Popular mas disputado', 'Saturado', 'Nicho'], typical: 'jogadores / mapa típico',
    earnings: 'Ganhos de outubro', roles: ['Código · 40%', 'Mapa · 35%', 'Música · fixo', 'Restante'], noTransfer: 'O POP calcula, não faz nenhuma transferência.',
  },
  wl: {
    eyebrow: 'Beta fechado', title: 'Entrar na lista de espera', intro: 'Estamos abrindo o POP aos poucos para acompanhar cada criador. Conte quem você é e enviamos seu acesso o quanto antes.',
    pageIntro: 'Durante o beta, as contas do POP são criadas pela equipe. Conte quem você é: quando seu acesso for aprovado, você recebe seu usuário e uma senha provisória.',
    invalid: 'Este link de acesso é inválido, já foi usado ou expirou. Peça de novo ou escreva para a equipe do POP.',
    name: 'Nome ou apelido', email: 'E-mail', studio: 'Estúdio', optional: '(opcional)', studioPh: 'Nome do estúdio', team: 'Tamanho da equipe',
    teams: ['Sozinho', '2 a 3', '4 a 10', 'Mais de 10'], exp: 'Experiência', exps: ['Iniciante no UEFN', 'Alguns mapas publicados', 'Criador experiente', 'Estúdio profissional'],
    links: 'Seus mapas ou redes', linksPh: 'Códigos de ilha, TikTok, YouTube…', msg: 'O que você quer fazer com o POP', msgPh: 'Ex.: somos 4 num mapa tycoon e nos perdemos entre o Discord e os Google Docs.',
    send: 'Pedir acesso', sending: 'Enviando…', nospam: 'Sem spam: só escrevemos para liberar seu acesso.', privacy: 'Política de privacidade',
    okTitle: 'Anotado, obrigado!', okText: 'Você vai receber um e-mail no endereço informado assim que seu acesso estiver pronto, com um link para ativar sua conta. Confira também a pasta de spam.',
    alreadyTitle: 'Você já está na lista.', approvedTitle: 'Seu acesso já foi aprovado.', approvedText: 'Sua conta está pronta: você recebeu um e-mail com um link para ativá-la e escolher sua senha. Confira também a pasta de spam.', close: 'Fechar',
  },
}
export default ptBR
