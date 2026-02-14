# Manual do Usuario - Dengine (PT-BR)

## 1. Visao geral
O Dengine e um editor 3D com foco em:
- gerenciamento de projeto e assets
- viewport 3D com gizmo
- hierarquia de objetos
- inspetor de componentes
- sistema de controle por nos (Fios)
- controlador de animacao
- terminal integrado (TerminAI)

Formato de projeto:
- arquivo principal: `.deng`
- estrutura de trabalho: pasta do projeto com subpastas como `Assets/`.

---

## 2. Hub (inicio)
Ao abrir o app, o Hub permite:
- `Novo Projeto`
- `Abrir .deng`
- `Atualizar Lista`
- listar projetos locais encontrados
- abrir projeto por duplo clique ou botao `Abrir`
- visualizar engines instaladas, versao atual/disponivel, atualizar/remover

Quando um projeto e aberto, o editor principal aparece.

---

## 3. Janela principal
### 3.1 Barra superior
- botoes de janela: minimizar, maximizar, fechar
- seletor de idioma: Portugues, English, Espanol
- menu `Arquivo`:
  - `Novo`
  - `Salvar`
  - `Importar`
  - `Sair`
- menu `Editar`:
  - `Undo (Ctrl+Z)`
  - `Redo (Ctrl+Shift+Z)`
- menu `Ajuda`:
  - `Sobre`

### 3.2 Barra de modo
- `Cena` e `Game`
- controles de execucao: play/pause e stop

### 3.3 Barra inferior (dock)
Atalhos de paineis:
- `Projeto`
- `Rig`
- `Animador`
- `Fios`
- `Log`
- `Git`
- `TerminAI`

Obs.: `Fios` ocupa a area central no lugar da viewport quando ativado.

---

## 4. Painel Projeto
O painel Projeto fica acoplado embaixo e pode ser redimensionado.

### 4.1 Recursos principais
- busca (`Buscar em Assets`)
- importacao de arquivo (`Importar`)
- menu de contexto no grid:
  - criar `Script C#`
  - criar `Material`
  - criar `Pasta`
  - importar
- breadcrumb de navegacao (`Assets`, `Packages`, subpastas)

### 4.2 Estrutura de pastas suportada
Assets:
- `Animations`
- `Materials`
- `Meshes`
- `Mold`
- `Scripts`

Packages:
- `TextMeshPro`
- `InputSystem`

### 4.3 Miniaturas e FBX expandido
No grid de assets:
- arquivos mostram thumbnail
- FBX tem botao de expandir no thumbnail
- expandido mostra sub-thumbnails (ex.: `Mesh`, `Esqueleto`, `Animacoes`, `Anim: <clip>`)
- sub-thumbnails suportam selecao por clique e feedback visual

Na arvore lateral (quando em `Meshes`):
- FBX tambem expande para `Mesh`, `Esqueleto` e lista de clips de animacao.

### 4.4 Arrastar e soltar (drag and drop)
Voce pode arrastar assets do Projeto para:
- viewport (instanciar objeto)
- hierarquia (criar objeto)
- aba `Controlador de animacao` em Fios

Feedback visual de arrasto:
- overlay com nome do item
- highlight da area de drop (viewport/hierarquia/fios)

---

## 5. Importacao e fluxo de animacao FBX
Ao importar FBX:
- arquivo vai para `Assets/Meshes` (com nome unico se necessario)
- o sistema detecta clipes de animacao e esqueleto
- e gerado automaticamente um modulo padrao `.animodule` em:
  - `Assets/Animations/Modules/`

Esse modulo padrao inclui:
- `skeleton_key`
- `source_fbx`
- mapeamento inferido (quando possivel):
  - `state.idle`
  - `state.walk`
  - `state.run`
  - `state.jump`
- lista de `clip=<fbx>::<clip>`

---

## 6. Hierarquia
A Hierarquia pode ficar dockada na esquerda/direita ou flutuando.

### 6.1 Operacoes
- selecionar objeto
- arrastar objetos para reorganizar
- menu de contexto por objeto
- delete por teclado (`Delete`/`Backspace`)

### 6.2 Criacao rapida (menu de contexto em area vazia)
- submenu `3D`:
  - Cube
  - Sphere
  - Cone
  - Cylinder
  - Plane
- submenu `Luzes`

### 6.3 Drag and drop de assets
Soltar um asset na Hierarquia cria um objeto com nome derivado do arquivo.

---

## 7. Viewport
A viewport suporta navegacao estilo Unity e gizmos.

### 7.1 Navegacao
- `Alt + LMB`: orbita
- `RMB`: olhar/orbita (dependendo do modo)
- `MMB`: pan
- `Alt + RMB`: zoom dolly
- `Scroll`: zoom
- Touchpad:
  - 2 dedos: pan
  - pinch: zoom
  - `Ctrl + 2 dedos`: orbita

### 7.2 Atalhos de vista
- `Num1`: frente
- `Num3`: lado
- `Num7`: topo
- `F`: focar objeto selecionado

### 7.3 Ferramentas
- alterna `3D/2D`
- alterna `Persp/Ortho`
- orientacao `Local/Global`
- gizmo de `Move`, `Rotate`, `Scale`
- modo `Move View` (pan de camera)

### 7.4 Cena e objeto
- selecao por clique
- transformacoes aplicadas via Inspector
- suporta import de malha FBX/OBJ/GLB/GLTF com pipeline e cache

---

## 8. Inspetor
Painel de propriedades do objeto selecionado.

### 8.1 Transform
- `Posicao`, `Rotacao`, `Escala`
- ON/OFF de componente
- `Aplicar Transformacoes` (bake e reset local)

### 8.2 Adicao de componentes
Botao `Componente` permite adicionar:
- `Fios Controller`
- `Rigidbody`
- `Animator`

### 8.3 Fios Controller
Campos:
- `Ativo` ON/OFF
- `Move Speed`
- `Rotate Speed`
- `Action Speed`

### 8.4 Rigidbody
Campos:
- `Ativo` ON/OFF
- `Gravity`
- `Jump Impulse`

### 8.5 Animator
Campos:
- `Ativo` ON/OFF
- `Modulo` (lista `.animodule`)
- `Aplicar modulo` (define clipe default)
- `Controller` (`.animctrl`/`.controller`)
- `Animacao` (lista de clipes FBX)

---

## 9. Fios
Quando ativo, substitui a viewport central.

Tabs:
- `Modulos`
- `Fios`
- `Controlador de animacao`

### 9.1 Tab Modulos
Configura entradas e modos de controle.

### 9.2 Tab Fios (grafo de nos)
Editor de nos para logica de controle.

Recursos principais:
- criar/remover nos
- conectar portas
- selecao multipla
- marquee (arraste em area vazia)
- renomear no (`F2`)
- deletar selecao (`Delete`/`Backspace`)
- cortar fios com gesto (Alt + botao direito arrastando)
- zoom e pan do canvas

Saidas de interesse:
- movimento
- look
- action
- comando de animacao (`PlayPause`, `Next`, `Prev`)

### 9.3 Tab Controlador de animacao
Canvas para estados de animacao.

Fluxo:
- lista de `Clipes` na coluna esquerda
- arraste clipe para o canvas para criar estado
- duplo clique em clipe tambem cria estado
- conecte estados clicando na saida de um no e depois na entrada de outro
- mover estado por drag
- `Atualizar` recarrega cache de clipes
- `Limpar` limpa canvas de estados

Integracao de arrasto do Projeto:
- soltar `Anim: <clip>` cria estado
- soltar `Animacoes (N)` ou FBX cria multiplos estados conforme clipes do arquivo

---

## 10. Execucao em Game/Play
Com `Play` ativo:
- `Fios Controller` aplica movimento/rotacao/acao em objetos ativos
- `Rigidbody` aplica gravidade e impulso de pulo
- `Animator` usa clipes configurados
- comandos de animacao vindos de Fios podem alternar play/pause e trocar clipe

---

## 11. TerminAI
Janela separada (viewport propria) para CLI integrada.

Modelos:
- `Qwen CLI`
- `Gemini CLI`
- `Codex CLI`

Fluxo:
1. seleciona modelo
2. verifica dependencias
3. provisiona ambiente/CLI
4. abre sessao de terminal no projeto atual

Recursos:
- terminal virtual com parser ANSI
- entrada de teclado/paste
- painel de log completo (colapsavel)

---

## 12. Undo/Redo e atalhos globais
- `Ctrl + Z`: Undo
- `Ctrl + Shift + Z`: Redo
- `Ctrl + Y`: Redo alternativo

Na Hierarquia:
- `Delete`/`Backspace`: remove selecionado

No Fios:
- `F2`: renomear no
- `Delete`/`Backspace`: remover no(s)

---

## 13. Salvamento de projeto
Ao salvar:
- arquivo `.deng` e gerado/atualizado
- cabecalho atual: `DENG1`
- lista assets como linhas `asset=<caminho_relativo>`

Se salvar fora de uma pasta de projeto:
- o app cria estrutura com `Assets/` automaticamente.

---

## 14. Arquivos auxiliares gerados pelo editor
Na raiz do workspace podem existir:
- `.dengine_fios_controls.cfg`
- `.dengine_fios_graph.cfg`
- `.dengine_fios.lua`
- `.dengine_hub_projects.txt`

Em `Assets/Animations/Modules/`:
- modulos `.animodule` (incluindo os gerados automaticamente no import FBX)

---

## 15. Boas praticas de fluxo
- importe FBX em `Assets/Meshes`
- confira sub-thumbnails (`Mesh`, `Esqueleto`, `Animacoes`)
- arraste clipes para `Fios > Controlador de animacao`
- no Inspetor, configure `Animator` com `Modulo` e `Animacao`
- use `Play` para validar locomocao e transicoes

---

## 16. Limitacoes atuais (estado atual do projeto)
- o controlador de animacao em Fios e funcional para criacao/ligacao de estados, mas ainda e um fluxo em evolucao
- alguns componentes/abas podem ter ajustes visuais incrementais entre builds
- parte dos dados de editor avancado pode depender de arquivos de configuracao locais

---

## 17. Resumo rapido (primeiros passos)
1. Abra o Hub e carregue um `.deng`.
2. Importe um FBX em `Projeto`.
3. Expanda o FBX e arraste um `Anim: ...` para `Fios > Controlador de animacao`.
4. Na Hierarquia, selecione o objeto.
5. No Inspetor, adicione `Animator` e `Fios Controller`.
6. Defina modulo/clipe no Animator.
7. Clique `Play` e teste movimento/animacao.
