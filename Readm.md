# README - Warnings do Compilador Rust

## Status

O projeto foi compilado com sucesso.

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.19s
```

Os avisos (`warnings`) exibidos pelo compilador **não impedem a execução do programa**. Eles apenas informam que existem partes do código que ainda não estão sendo utilizadas.

---

# Principais avisos

## rodando teste de imagem
--log -- 
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.45s
     Running `target/debug/captura-tela-video`
2026-08-04T01:53:19.693997Z  INFO captura_tela_video: 🎥 VibesVR Screen Capture - PipeWire Edition
2026-08-04T01:53:19.694057Z  INFO captura_tela_video: ============================================
2026-08-04T01:53:19.694078Z  INFO captura_tela_video: 
2026-08-04T01:53:19.702389Z  INFO captura_tela_video: 🪟 Criando janela
2026-08-04T01:53:19.769024Z  WARN wgpu_hal::vulkan::instance: InstanceFlags::VALIDATION requested, but unable to find layer: VK_LAYER_KHRONOS_validation
2026-08-04T01:53:19.786186Z  INFO wgpu_hal::vulkan::instance: Debug utils not enabled: debug_utils_user_data not passed to Instance::from_raw
2026-08-04T01:53:19.791665Z  INFO wgpu_hal::gles::egl: Using Wayland platform
MESA-INTEL: warning: Haswell Vulkan support is incomplete
2026-08-04T01:53:19.845666Z  WARN wgpu_hal::vulkan::adapter: Adapter is not Vulkan compliant, hiding adapter: Intel(R) HD Graphics 4400 (HSW GT2)
2026-08-04T01:53:19.856779Z  INFO wgpu_core::instance: Adapter Vulkan AdapterInfo { name: "AMD Radeon RX 570 Series (RADV POLARIS10)", vendor: 4098, device: 26591, device_type: DiscreteGpu, driver: "radv", driver_info: "Mesa 26.1.5", backend: Vulkan }
2026-08-04T01:53:19.926336Z  INFO captura_tela_video: ✅ GPU inicializada: 1280x720
2026-08-04T01:53:19.926367Z  INFO captura_tela_video: 🚀 Inicializando captura de tela...
2026-08-04T01:53:19.926413Z  INFO captura_tela_video::capture: 🚀 Iniciando captura de tela...
2026-08-04T01:53:19.926432Z  INFO captura_tela_video::portal: 📱 Solicitando permissão via xdg-desktop-portal...
2026-08-04T01:53:19.926449Z  INFO captura_tela_video::portal: ✅ Portal autorizado: 1920x1080
2026-08-04T01:53:19.926462Z  INFO captura_tela_video::capture: ✅ Portal conectado
2026-08-04T01:53:19.926472Z  INFO captura_tela_video::pipewire::context: 🔧 Inicializando PipeWire 0.10...
2026-08-04T01:53:19.926487Z  INFO captura_tela_video::pipewire::context: ✅ PipeWire 0.10 inicializado (simulado)
2026-08-04T01:53:19.926500Z  INFO captura_tela_video::capture: ✅ PipeWire inicializado
2026-08-04T01:53:19.926514Z  INFO captura_tela_video::pipewire::stream: 📹 Criando stream PipeWire: 1920x1080 @ 60fps
2026-08-04T01:53:19.926527Z  INFO captura_tela_video::capture: ✅ Stream criado
2026-08-04T01:53:19.926547Z  INFO captura_tela_video: ✅ Capturador inicializado: 1920x1080
2026-08-04T01:53:19.926566Z  INFO captura_tela_video: ✅ Sistema pronto! Pressione ESC para sair
2026-08-04T01:53:19.926581Z  INFO captura_tela_video: ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
2026-08-04T01:53:20.777639Z  INFO captura_tela_video: 📊 FPS: 5
2026-08-04T01:53:21.803325Z  INFO captura_tela_video: 📊 FPS: 6
2026-08-04T01:53:22.861594Z  INFO captura_tela_video: 📊 FPS: 6
2026-08-04T01:53:23.966476Z  INFO captura_tela_video: 📊 FPS: 6
2026-08-04T01:53:24.811110Z  INFO captura_tela_video: 🛑 Fechando janela...
2026-08-04T01:53:24.965808Z  INFO captura_tela_video::pipewire::stream: 📹 Stream PipeWire parado
2026-08-04T01:53:24.965838Z  INFO captura_tela_video::capture: 🛑 Captura parada
2026-08-04T01:53:24.965852Z  INFO captura_tela_video::pipewire::context: 🔧 PipeWire finalizado
2026-08-04T01:53:24.965863Z  INFO captura_tela_video::pipewire::stream: 📹 Stream PipeWire parado

## 1. Campo nunca utilizado (`field is never read`)

Exemplo:

```text
warning: field `streams` is never read
```

Significa que a estrutura possui um campo chamado `streams`, porém em nenhum momento ele é acessado.

Exemplo:

```rust
pub struct PortalSession {
    streams: Vec<u32>,
}
```

Se nunca existir algo como:

```rust
session.streams
```

o compilador gera esse aviso.

---

## 2. Método nunca utilizado (`method is never used`)

Exemplo:

```text
warning: method `streams` is never used
```

O método foi implementado, mas nenhuma parte do projeto o chama.

Exemplo:

```rust
pub fn streams(&self) -> &[u32] {
    &self.streams
}
```

Caso nunca exista:

```rust
session.streams();
```

o compilador emitirá esse warning.

---

## 3. Struct nunca construída (`struct is never constructed`)

Exemplo:

```text
warning: struct `VideoBuffer` is never constructed
```

A estrutura existe no código, porém nunca foi criada.

Exemplo:

```rust
pub struct VideoBuffer { ... }
```

Mas nunca ocorre:

```rust
let buffer = VideoBuffer::new(...);
```

---

## 4. Enum nunca utilizado (`enum is never used`)

Exemplo:

```text
warning: enum `PixelFormat` is never used
```

O enum foi criado para uso futuro, porém ainda não participa do fluxo do programa.

---

## 5. Função nunca utilizada

Exemplo:

```text
warning: method `frames_encoded` is never used
```

A função existe:

```rust
pub fn frames_encoded(&self) -> u64
```

Mas nunca é chamada.

---

## 6. Campos da struct nunca utilizados

Exemplo:

```text
warning: fields `stride`, `timestamp` e `frame_number` are never read
```

Os dados são armazenados dentro da estrutura `Frame`, porém atualmente apenas:

* width
* height
* data

estão sendo utilizados.

Os demais campos provavelmente serão utilizados quando houver transmissão de vídeo em tempo real.

---

# Isso é um erro?

**Não.**

Warnings são apenas recomendações do compilador.

Enquanto não existir nenhuma mensagem como:

```text
error[E....]
```

o programa foi compilado corretamente.

---

# Por que esses warnings aparecem?

Durante o desenvolvimento é comum implementar a arquitetura antes de utilizar todas as funções.

Neste projeto vários componentes já estão preparados para etapas futuras:

* Portal PipeWire
* Captura de tela
* Buffer de vídeo
* Encoder H.264
* Streaming
* Cliente VR

Como essas etapas ainda não estão totalmente conectadas, algumas estruturas permanecem sem uso.

---

# É recomendado removê-los?

Não neste momento.

Esses avisos desaparecerão naturalmente conforme o projeto evoluir.

Somente vale remover ou adicionar:

```rust
#[allow(dead_code)]
```

quando houver certeza de que determinado código ficará propositalmente sem uso.

---

# Próximas etapas do projeto

* Finalizar captura via PipeWire.
* Validar os frames capturados.
* Codificar todos os frames em H.264.
* Implementar servidor de streaming.
* Criar protocolo de transmissão.
* Desenvolver cliente Android.
* Exibir o vídeo em realidade virtual.

---

# Resumo

| Situação   | Status             |
| ---------- | ------------------ |
| Compilação | ✅ Sucesso          |
| Erros      | ✅ Nenhum           |
| Warnings   | 12                 |
| Execução   | Pronta para testes |

Os warnings atuais são esperados para um projeto em desenvolvimento e não impedem a execução da aplicação.
