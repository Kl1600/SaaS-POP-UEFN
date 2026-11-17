import type { Dict } from './fr'
// Português de Portugal (dérivé de pt-BR).
const ptPT: Dict = {
  meta: { title: 'POP · O SaaS dos criadores de UEFN' },
  nav: { features: 'Funcionalidades', pricing: 'Preços', faq: 'FAQ', login: 'Entrar', join: 'Entrar na beta', home: 'POP, início', lang: 'Idioma', search: 'Procurar um país' },
  hero: {
    announce: 'Beta fechada aberta a criadores de UEFN', h1a: 'Crie no UEFN.', h1b: 'Sozinho ou em equipa, sem caos.',
    subA: 'O POP é o SaaS que ajuda os criadores de UEFN da ideia ao lançamento: ', subB: 'tarefas de uma hora', subC: ', quadro branco, Organizador ligado ao UEFN, IAs ligadas, análise de mercado e divisão de ganhos.',
    cta: 'Entrar na lista de espera', see: 'Ver as funcionalidades', micro1: 'Beta fechada', micro2: 'Acesso por lista de espera', micro3: 'Ligado ao UEFN',
  },
  marquee: { label: 'Funciona com as suas ferramentas', epic: 'API do Fortnite da Epic' },
  pain: {
    eyebrow: 'O problema', h2a: 'Discord, Google Docs, ficheiros por todo o lado…', h2b: 'e o mapa não avança.', without: 'Sem POP', with: 'Com POP',
    bad: ['“Quem mexeu no spawner?”', 'As ideias perdem-se no Discord', 'Uma lista de tarefas num Google Doc', 'A folha de cálculo dos ganhos, versão 7', '“Lançamos quando, afinal?”', 'Construir sem saber se o género funciona'],
    good: ['Cada ação é registada e pode ser desfeita', 'As ideias do quadro tornam-se tarefas', 'Tarefas de uma hora, com dependências', 'Um extrato claro para cada um', 'Marcos e atrasos acompanhados automaticamente', 'Um veredito por género, com números reais'],
  },
  feat: {
    eyebrow: 'Funcionalidades', h2: 'Tudo para criar no UEFN, da ideia ao dia do lançamento.', p: 'O que vê aqui são os ecrãs reais do POP, simplificadas.',
    items: [
      { tag: 'Tarefas e planeamento', h: 'Tarefas de uma hora. Um planeamento que se cumpre.', p: 'O gerador divide o mapa em tarefas de uma hora com as suas dependências. O planeamento acompanha os marcos e assinala atrasos, e um assistente integrado resume o dia.', pts: ['Kanban e dependências', 'Marcos e atrasos acompanhados', 'Resumo do dia'] },
      { tag: 'Organizador', h: 'Todos os seus dispositivos. Organizados em três segundos.', p: 'Procure por tag, tipo ou pasta, selecione vinte e renomeie, mova ou duplique de uma vez. Ctrl+Z desfaz. Se o UEFN estiver fechado, as mudanças esperam.', pts: ['Pesquisa tag:, type:, pasta:', 'Edição em lote', 'Tudo pode ser desfeito'] },
      { tag: 'Quadro branco', h: 'Desenhe o mapa em equipa.', p: 'Post-its, formas, setas, cores, negrito, sublinhado: tudo para organizar as ideias. Votem e transformem os melhores post-its em tarefas.', pts: ['Formatação completa', 'Tempo real em equipa', 'Post-its → tarefas'] },
      { tag: 'Análise', h: 'Saiba o que construir antes de construir.', p: 'Quais géneros atraem jogadores, onde há espaço, quais mapas decolam no lançamento e o que está a dar em vídeo. Números da API oficial da Epic.', pts: ['Veredito por género', 'Mapas que funcionam', 'Tendências TikTok e Shorts'] },
      { tag: 'Pagamentos', h: 'Cada um sabe quanto recebe.', p: 'Regras de divisão por porcentagem, valor fixo ou por tarefa, importação de ganhos, extrato por pessoa. Acabou a folha de cálculo perdida no Discord.', pts: ['Porcentagem, fixo, por tarefa', 'Importação CSV', 'Extrato de cada um'] },
    ],
    aiB: 'Já usa uma IA?', aiText: 'Ligue Claude, Cursor, Codex ou outra IA compatível com MCP: ela assume uma tarefa, escreve o Verse, compila e fecha a tarefa com uma nota. Cada ação é registada e pode ser desfeita.',
  },
  stats: [['1 h', 'por tarefa, para avançar todos os dias'], ['0', 'serviço de IA externo obrigatório'], ['7 dias', 'de estatísticas por mapa, da API da Epic'], ['100%', 'das ações no UEFN podem ser desfeitas']],
  pricing: {
    eyebrow: 'Preços', h2: 'Escolha o seu plano.', p: 'O POP está em beta fechada: o acesso é por lista de espera. Os preços valem no lançamento, e será avisado antes de qualquer pagamento.',
    perMonth: '/ mês', compare: 'Comparar planos', unlimited: 'Ilimitado', soon: 'Em breve',
    plans: [
      { name: 'Criador', tag: 'Para começar', desc: 'Um mapa, uma equipa pequena, o essencial.', cta: 'Entrar na lista' },
      { name: 'Studio', tag: 'O mais completo', desc: 'Para estúdios que lançam mapas em equipa.', cta: 'Entrar na lista' },
      { name: 'Studio+', tag: 'Em breve', desc: 'Tudo do Studio, mais o POP Forge: a aplicação de computador que constrói no UEFN.', cta: 'Entrar na lista' },
    ],
    rows: ['Organização', 'Tarefas de uma hora, Kanban, dependências', 'Assistente integrado (resumo do dia, atrasos)', 'Páginas, quadro branco, planejamento e marcos', 'Mapas', 'Membros', 'UEFN', 'UEFN Link: projetos sincronizados', 'Organizador de atores e edição em lote', 'IAs ligadas (Claude, Cursor, Codex…)', 'POP Forge: construção assistida no UEFN', 'Crescimento', 'Análise de mercado e mapas que funcionam', 'Tendências de vídeo TikTok e YouTube Shorts', 'Troca de playtests e dia do lançamento', 'Divisão de ganhos e extratos', 'Discord, GitHub, Lore'],
  },
  faq: {
    eyebrow: 'FAQ', h2: 'As perguntas que nos fazem.',
    items: [
      ['Preciso de um PC com Windows?', 'Para ligar o UEFN, sim: o UEFN só corre no Windows, e o UEFN Link corre nesse PC. O resto do POP funciona em qualquer navegador, inclusive no Mac.'],
      ['O POP publica o meu mapa?', 'Não. O POP prepara tudo (verificação antes da publicação, playtests, plano do dia do lançamento), mas a publicação é sempre feita no portal de criadores da Epic.'],
      ['O POP funciona se eu criar sozinho?', 'Sim. O POP serve tanto quem cria sozinho como equipas: o plano Criador é gratuito para um mapa e pode convidar membros mais tarde sem mudar nada.'],
      ['E se eu quiser usar a minha própria IA?', 'Ligue Claude, Cursor, Codex ou outra IA compatível com MCP em “IAs ligadas”. Ela pode ler e editar as suas tarefas, o seu Verse e os seus atores no UEFN, e cada ação é registada e pode ser desfeita.'],
      ['De onde vêm os números da análise?', 'Da API oficial do Fortnite da Epic: jogadores, pico de jogadores simultâneos, tempo de jogo e retenção de cada mapa público, por 7 dias.'],
      ['Como consigo acesso?', 'O POP está em beta fechada. Entre na lista de espera: analisamos os pedidos um a um e depois enviamos o seu utilizador e uma palavra-passe provisória, que muda no primeiro acesso.'],
      ['Quanto custa?', 'Os preços estão mais acima. Eles valem no lançamento do POP; os membros da beta são avisados antes de qualquer pagamento.'],
    ],
  },
  final: { h2a: 'O seu próximo mapa', h2b: 'começa aqui.', p: 'Entre na lista de espera: enviamos o seu acesso assim que abrir uma vaga.' },
  foot: { made: 'Feito por criadores de Fortnite, para criadores de Fortnite.', legal: 'Fortnite e UEFN são marcas da Epic Games. O POP não é afiliado nem endossado pela Epic Games.', product: 'Produto', help: 'Ajuda', account: 'Conta' },
  mock: {
    overview: 'Visão geral', nav: ['Visão geral', 'Páginas', 'Quadro branco', 'Tarefas', 'Organizador', 'Mercado'],
    hello: 'Olá Léa, sugiro começar pela #42.', late: '2 tarefas atrasadas (a mais antiga era para 12 de nov.).', mile: 'Marco “Beta” em 5 dias: faltam 9 tarefas, atraso previsto de 1 dia.',
    take: 'Assumir', start: 'Começar', unlocks: 'desbloqueia outras 3 tarefas', today: 'prevista para hoje', waits: 'aguarda #42',
    todo: 'A fazer', doing: 'Em curso', done: 'Feito', tasks: ['Sistema de pontuação', 'Cenário da zona 2', 'Ecrã final', 'Playtest interno', 'Spawn dos jogadores', 'Tempestade'],
    sent: 'enviado ao UEFN', err: 'Linha 12: int e float não podem ser somados.', fix: 'Corrigir: Points += 1',
    red: 'Equipa vermelha', selected: '2 selecionados', rename: 'Renomear', move: 'Mover', dup: 'Duplicar',
    s1: 'Modo equipas 4v4', s2a: 'Chefe final', s2b: 'no centro', s3: 'Economia: 1 moeda / eliminação',
    verdicts: ['Boa oportunidade', 'Popular mas disputado', 'Saturado', 'Nicho'], typical: 'jogadores / mapa típico',
    earnings: 'Ganhos de outubro', roles: ['Código · 40%', 'Mapa · 35%', 'Música · fixo', 'Restante'], noTransfer: 'O POP calcula, não faz nenhuma transferência.',
  },
  wl: {
    eyebrow: 'Beta fechada', title: 'Entrar na lista de espera', intro: 'Estamos a abrir o POP aos poucos para acompanhar cada criador. Diga-nos quem é e enviamos o seu acesso o quanto antes.',
    pageIntro: 'Durante a beta, as contas do POP são criadas pela equipa. Diga-nos quem é: quando o seu acesso for aprovado, recebe o seu utilizador e uma palavra-passe provisória.',
    invalid: 'Este link de acesso é inválido, já foi usado ou expirou. Peça de novo ou escreva à equipa do POP.',
    name: 'Nome ou apelido', email: 'E-mail', studio: 'Estúdio', optional: '(opcional)', studioPh: 'Nome do estúdio', team: 'Tamanho da equipa',
    teams: ['Sozinho', '2 a 3', '4 a 10', 'Mais de 10'], exp: 'Experiência', exps: ['Iniciante no UEFN', 'Alguns mapas publicados', 'Criador experiente', 'Estúdio profissional'],
    links: 'Os seus mapas ou redes', linksPh: 'Códigos de ilha, TikTok, YouTube…', msg: 'O que quer fazer com o POP', msgPh: 'Ex.: somos 4 num mapa tycoon e perdemo-nos entre o Discord e os Google Docs.',
    send: 'Pedir acesso', sending: 'A enviar…', nospam: 'Sem spam: só escrevemos para liberar o seu acesso.',
    okTitle: 'Anotado, obrigado!', okText: 'O seu pedido chegou à equipa do POP. Assim que houver uma vaga, enviamos o seu utilizador e uma palavra-passe provisória.',
    alreadyTitle: 'Já está na lista.', approvedTitle: 'O o seu acesso já foi aprovado.', approvedText: 'A sua conta está pronta: a equipa do POP vai enviar-lhe o utilizador e a palavra-passe provisória. Se não chegar, escreva-nos.', close: 'Fechar',
  },
}
export default ptPT
