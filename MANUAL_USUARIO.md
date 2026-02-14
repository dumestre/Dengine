# Dengine - User Manual / Manual do Usuario / Manual de Usuario

## PT-BR
### 1) Visao geral
O Dengine e um editor com foco em cena 3D, hierarquia, inspetor, projeto, hub e terminal integrado (TerminAI).

### 2) Hub
- Cria e abre projetos `.deng`.
- Lista projetos locais recentes.
- Mostra engine instalada, versao atual e versao disponivel.
- Acoes de atualizar/remover engine (quando aplicavel).

### 3) Janela principal
- Barra superior customizada (menus Arquivo/Editar/Ajuda e selecao de idioma).
- Barra inferior com botoes:
  - Projeto
  - Rig
  - Animador
  - Fios
  - Log
  - Git
  - TerminAI

### 4) Projeto
- Importacao de assets (modelos, imagens etc.).
- Grade de arquivos com miniaturas.
- Salvar projeto em `.deng`.

### 5) Hierarquia
- Objetos padrao: `Directional Light` e `Main Camera`.
- Selecao de objeto por clique.
- Menu de contexto por objeto.
- Delete/Backspace com confirmacao.
- Clique direito em area vazia:
  - submenu `3D` (Cube, Sphere, Cone, Cylinder, Plane)
  - submenu de luzes.

### 6) Viewport
- Navegacao de camera:
  - Alt + LMB: orbita
  - RMB: look/orbita conforme modo
  - MMB: pan
  - Scroll: zoom
  - Touchpad: 2 dedos pan, Ctrl + 2 dedos orbita
- Atalhos de vista numerico:
  - Num1 frente
  - Num3 lado
  - Num7 topo
- Gizmo:
  - Move/Rotate/Scale
  - Local/Global
- Selecao visual com outline.
- Render com pipeline GPU e depth buffer.

### 7) Inspetor
- Componente Transform (posicao, rotacao, escala).
- Aplicar Transformacoes (bake na malha e reset do transform local).
- On/Off de componente.

### 8) Undo/Redo
- Undo: Ctrl + Z
- Redo: Ctrl + Shift + Z (e Ctrl + Y como alternativo)

### 9) TerminAI
- Janela separada da main.
- Escolha de modelo:
  - Qwen CLI
  - Gemini CLI
  - Codex CLI
- Fluxo:
  1. verifica dependencias (Node/npm e CLI)
  2. instala quando necessario
  3. abre sessao no shell do sistema
  4. entra na raiz do projeto aberto
  5. executa o modelo
- Output selecionavel.

### 10) Formato de projeto
- Extensao oficial: `.deng`.

### 11) Fios (Nodes)
- Conexao de fios com area de clique ampliada nos conectores (entrada/saida), facilitando o plug.
- Selecao multipla:
  - Shift + clique para adicionar/remover da selecao.
  - Arraste em area vazia para selecao em caixa (marquee).
- Renomear bloco:
  - botao `Renomear` + `Aplicar Nome`
  - atalho `F2` para iniciar renomeacao.
- Cortar fios:
  - segure `Alt` e arraste com botao direito sobre os fios para cortar conexoes cruzadas.
- Exclusao:
  - `Delete`/`Backspace` remove o(s) bloco(s) selecionado(s) e suas conexoes.

---

## EN
### 1) Overview
Dengine is a 3D editor focused on scene editing, hierarchy, inspector, project assets, hub, and integrated terminal (TerminAI).

### 2) Hub
- Create/open `.deng` projects.
- List local recent projects.
- Show installed engine, current version, and available version.
- Update/remove engine actions (when available).

### 3) Main window
- Custom top bar (File/Edit/Help and language selector).
- Bottom bar buttons:
  - Project
  - Rig
  - Animator
  - Wires
  - Log
  - Git
  - TerminAI

### 4) Project panel
- Asset import (models, images, etc.).
- File grid with thumbnails.
- Save project as `.deng`.

### 5) Hierarchy
- Default objects: `Directional Light` and `Main Camera`.
- Click to select objects.
- Per-object context menu.
- Delete/Backspace with confirmation.
- Right-click empty area:
  - `3D` submenu (Cube, Sphere, Cone, Cylinder, Plane)
  - lights submenu.

### 6) Viewport
- Camera navigation:
  - Alt + LMB: orbit
  - RMB: look/orbit depending on mode
  - MMB: pan
  - Scroll: zoom
  - Touchpad: 2-finger pan, Ctrl + 2-finger orbit
- Numeric view hotkeys:
  - Num1 front
  - Num3 side
  - Num7 top
- Gizmo:
  - Move/Rotate/Scale
  - Local/Global
- Outline-based visual selection.
- GPU rendering path with depth buffer.

### 7) Inspector
- Transform component (position, rotation, scale).
- Apply Transforms (mesh bake and local transform reset).
- Component On/Off toggle.

### 8) Undo/Redo
- Undo: Ctrl + Z
- Redo: Ctrl + Shift + Z (Ctrl + Y alternate)

### 9) TerminAI
- Separate window from main editor.
- Model chooser:
  - Qwen CLI
  - Gemini CLI
  - Codex CLI
- Flow:
  1. verify dependencies (Node/npm and CLI)
  2. install if needed
  3. start system shell session
  4. cd into opened project root
  5. execute selected model
- Selectable output.

### 10) Project format
- Official project extension: `.deng`.

### 11) Wires (Nodes)
- Wire connection now uses larger input/output connector hit areas for easier plugging.
- Multi-selection:
  - Shift + click to add/remove nodes from selection.
  - Drag on empty canvas to marquee-select.
- Node rename:
  - `Rename` button + `Apply Name`
  - `F2` shortcut to start renaming.
- Wire cut:
  - hold `Alt` and drag with right mouse button across wires to cut crossed links.
- Deletion:
  - `Delete`/`Backspace` removes selected node(s) and connected links.

---

## ES
### 1) Resumen
Dengine es un editor 3D enfocado en escena, jerarquia, inspector, proyecto, hub y terminal integrado (TerminAI).

### 2) Hub
- Crear/abrir proyectos `.deng`.
- Listar proyectos locales recientes.
- Mostrar engine instalada, version actual y version disponible.
- Acciones de actualizar/eliminar engine (cuando aplique).

### 3) Ventana principal
- Barra superior personalizada (Archivo/Editar/Ayuda e idioma).
- Barra inferior:
  - Proyecto
  - Rig
  - Animador
  - Fios/Wires
  - Log
  - Git
  - TerminAI

### 4) Proyecto
- Importacion de assets (modelos, imagenes, etc.).
- Cuadricula con miniaturas.
- Guardar proyecto en `.deng`.

### 5) Jerarquia
- Objetos por defecto: `Directional Light` y `Main Camera`.
- Seleccion por clic.
- Menu contextual por objeto.
- Delete/Backspace con confirmacion.
- Clic derecho en area vacia:
  - submenu `3D` (Cube, Sphere, Cone, Cylinder, Plane)
  - submenu de luces.

### 6) Viewport
- Navegacion de camara:
  - Alt + LMB: orbita
  - RMB: look/orbita segun modo
  - MMB: pan
  - Scroll: zoom
  - Touchpad: 2 dedos pan, Ctrl + 2 dedos orbita
- Atajos numericos:
  - Num1 frente
  - Num3 lado
  - Num7 arriba
- Gizmo:
  - Move/Rotate/Scale
  - Local/Global
- Seleccion visual por contorno (outline).
- Render GPU con depth buffer.

### 7) Inspector
- Componente Transform (posicion, rotacion, escala).
- Aplicar Transformaciones (bake de malla y reset local).
- On/Off de componente.

### 8) Undo/Redo
- Undo: Ctrl + Z
- Redo: Ctrl + Shift + Z (Ctrl + Y alternativo)

### 9) TerminAI
- Ventana separada del editor principal.
- Selector de modelo:
  - Qwen CLI
  - Gemini CLI
  - Codex CLI
- Flujo:
  1. verificar dependencias (Node/npm y CLI)
  2. instalar si falta
  3. abrir sesion de shell del sistema
  4. entrar a la raiz del proyecto abierto
  5. ejecutar el modelo
- Salida seleccionable.

### 10) Formato de proyecto
- Extension oficial: `.deng`.

### 11) Fios/Wires (Nodos)
- Conexion de cables con area de clic ampliada en conectores de entrada/salida para facilitar el enlace.
- Seleccion multiple:
  - Shift + clic para agregar/quitar nodos de la seleccion.
  - Arrastrar en area vacia para seleccion por caja (marquee).
- Renombrar nodo:
  - boton `Renomear/Rename` + `Aplicar Nombre`
  - atajo `F2` para iniciar renombrado.
- Corte de cables:
  - mantener `Alt` y arrastrar con boton derecho sobre cables para cortar conexiones cruzadas.
- Eliminacion:
  - `Delete`/`Backspace` elimina nodo(s) seleccionado(s) y sus conexiones.
