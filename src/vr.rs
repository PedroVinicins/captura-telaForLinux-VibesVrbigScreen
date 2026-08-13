use std::time::{Duration, Instant};

use bevy::{
    app::AppExit,
    prelude::*,
    render::{
        camera::Viewport,
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    },
    window::{PresentMode, PrimaryWindow, WindowMode, WindowPlugin, WindowResized},
    winit::{UpdateMode, WinitSettings},
};
use tokio::runtime::Runtime;
use tracing::{info, warn};

use crate::capture::ScreenCapture;

// Comece em modo janela para a janela do VibesVR nao esconder a fonte
// selecionada no portal. F11 continua ativando a saida SBS em tela cheia.
const OUTPUT_WIDTH: f32 = 1280.0;
const OUTPUT_HEIGHT: f32 = 720.0;
const INITIAL_TEXTURE_WIDTH: u32 = 1920;
const INITIAL_TEXTURE_HEIGHT: u32 = 1080;

// Unidades do mundo 3D em metros.
const IPD_METERS: f32 = 0.064;
const EYE_HEIGHT: f32 = 1.20;
const EYE_Z: f32 = 1.50;
const FOV_DEGREES: f32 = 90.0;

const SCREEN_WIDTH: f32 = 8.53;
const SCREEN_HEIGHT: f32 = 4.80;
const SCREEN_Y: f32 = 1.20;
const SCREEN_Z: f32 = -6.00;

struct CaptureState {
    // A captura deve ser destruída antes do runtime.
    capture: ScreenCapture,
    _runtime: Runtime,
}

#[derive(Resource)]
struct DesktopScreen {
    // Trocar o identificador da textura junto com o material impede que o
    // renderizador continue usando o asset xadrez que já estava na GPU.
    current_texture: Handle<Image>,
    stale_texture: Option<Handle<Image>>,
    material: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct FrameStats {
    frames: u32,
    black_frames: u32,
    first_frame_uploaded: bool,
    last_report: Instant,
}

#[derive(Component, Clone, Copy)]
enum Eye {
    Left,
    Right,
}

#[derive(Component)]
struct CinemaScreen;

pub fn run(capture: ScreenCapture, runtime: Runtime) {
    let mut app = App::new();

    app.insert_non_send_resource(CaptureState {
        capture,
        _runtime: runtime,
    })
    // Captura de vídeo não pode ser suspensa quando a janela perde foco (por
    // exemplo, enquanto Sunshine ou o seletor do portal está em primeiro plano).
    .insert_resource(WinitSettings {
        focused_mode: UpdateMode::Continuous,
        unfocused_mode: UpdateMode::Continuous,
    })
    .insert_resource(ClearColor(Color::BLACK))
    .insert_resource(AmbientLight {
        color: Color::srgb(0.12, 0.14, 0.20),
        brightness: 55.0,
        ..default()
    })
    .insert_resource(FrameStats {
        frames: 0,
        black_frames: 0,
        first_frame_uploaded: false,
        last_report: Instant::now(),
    })
    .add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "VibesVR - Cinema SBS".into(),
            resolution: (OUTPUT_WIDTH, OUTPUT_HEIGHT).into(),
            // Nao abra diretamente em fullscreen no mesmo monitor capturado:
            // isso faria o PipeWire capturar a propria janela preta do VibesVR.
            mode: WindowMode::Windowed,
            present_mode: PresentMode::AutoVsync,
            resizable: true,
            ..default()
        }),
        ..default()
    }))
    .add_systems(Startup, setup_cinema)
    .add_systems(
        Update,
        (
            update_desktop_texture,
            update_eye_viewports,
            keyboard_controls,
        ),
    );

    info!("VibesVR SBS iniciado em modo janela: ESC sai; F11 alterna tela cheia");
    app.run();
}

fn setup_cinema(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let image = new_desktop_image(
        INITIAL_TEXTURE_WIDTH,
        INITIAL_TEXTURE_HEIGHT,
        test_pattern_rgba(INITIAL_TEXTURE_WIDTH, INITIAL_TEXTURE_HEIGHT),
    );
    let texture = images.add(image);

    let screen_material = materials.add(StandardMaterial {
        base_color_texture: Some(texture.clone()),
        unlit: true,
        cull_mode: None,
        ..default()
    });

    commands.insert_resource(DesktopScreen {
        current_texture: texture,
        stale_texture: None,
        material: screen_material.clone(),
    });

    let room_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.018, 0.022, 0.032),
        perceptual_roughness: 0.92,
        metallic: 0.02,
        ..default()
    });

    let floor_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.035, 0.040, 0.055),
        perceptual_roughness: 0.80,
        metallic: 0.08,
        ..default()
    });

    let border_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.003, 0.003, 0.005),
        perceptual_roughness: 0.98,
        ..default()
    });

    // Tela do desktop. Rectangle fica no plano XY e aponta para as câmeras em +Z.
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(Rectangle::new(SCREEN_WIDTH, SCREEN_HEIGHT)),
            material: screen_material,
            transform: Transform::from_xyz(0.0, SCREEN_Y, SCREEN_Z),
            ..default()
        },
        CinemaScreen,
    ));

    // Moldura/parede preta atrás da tela.
    commands.spawn(PbrBundle {
        mesh: meshes.add(Cuboid::new(SCREEN_WIDTH + 0.34, SCREEN_HEIGHT + 0.34, 0.16)),
        material: border_material.clone(),
        transform: Transform::from_xyz(0.0, SCREEN_Y, SCREEN_Z - 0.10),
        ..default()
    });

    // Sala escura: chão, teto, fundo e paredes laterais.
    spawn_box(
        &mut commands,
        &mut meshes,
        floor_material.clone(),
        Vec3::new(16.0, 0.18, 18.0),
        Vec3::new(0.0, -1.72, -2.0),
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        room_material.clone(),
        Vec3::new(16.0, 0.18, 18.0),
        Vec3::new(0.0, 4.25, -2.0),
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        room_material.clone(),
        Vec3::new(0.18, 6.0, 18.0),
        Vec3::new(-8.0, 1.25, -2.0),
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        room_material.clone(),
        Vec3::new(0.18, 6.0, 18.0),
        Vec3::new(8.0, 1.25, -2.0),
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        room_material,
        Vec3::new(16.0, 6.0, 0.18),
        Vec3::new(0.0, 1.25, -7.0),
    );

    // Luz fraca, apenas para a geometria do ambiente. A tela é unlit.
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            color: Color::srgb(0.25, 0.32, 0.55),
            intensity: 480.0,
            range: 14.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(0.0, 3.2, 0.0),
        ..default()
    });

    // Cabeça fixa. As câmeras ficam paralelas, separadas pelo IPD.
    commands
        .spawn(SpatialBundle::from_transform(Transform::from_xyz(
            0.0, EYE_HEIGHT, EYE_Z,
        )))
        .with_children(|head| {
            spawn_eye(head, Eye::Left, -IPD_METERS * 0.5, 0);
            spawn_eye(head, Eye::Right, IPD_METERS * 0.5, 1);
        });
}

fn spawn_eye(parent: &mut ChildBuilder, eye: Eye, x: f32, order: isize) {
    parent.spawn((
        Camera3dBundle {
            camera: Camera { order, ..default() },
            projection: PerspectiveProjection {
                fov: FOV_DEGREES.to_radians(),
                near: 0.05,
                far: 100.0,
                ..default()
            }
            .into(),
            transform: Transform::from_xyz(x, 0.0, 0.0),
            ..default()
        },
        eye,
    ));
}

fn spawn_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    size: Vec3,
    position: Vec3,
) {
    commands.spawn(PbrBundle {
        mesh: meshes.add(Cuboid::new(size.x, size.y, size.z)),
        material,
        transform: Transform::from_translation(position),
        ..default()
    });
}

fn update_desktop_texture(
    mut capture_state: NonSendMut<CaptureState>,
    mut desktop: ResMut<DesktopScreen>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<FrameStats>,
) {
    let Some(frame) = capture_state.capture.try_receive_frame() else {
        return;
    };

    let Some(expected) = usize::try_from(frame.width())
        .ok()
        .and_then(|width| {
            usize::try_from(frame.height())
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        warn!("Dimensões do frame excedem a plataforma");
        return;
    };
    if frame.data().len() != expected {
        warn!(
            received = frame.data().len(),
            expected, "Frame RGBA com tamanho inválido"
        );
        return;
    }

    // Se todos os frames forem pretos, a captura esta vendo a propria janela
    // do VibesVR ou a fonte escolhida no portal nao possui conteudo visivel.
    let frame_is_black = looks_black(frame.data());

    let signature = sampled_rgb_signature(frame.data());

    let Some(material) = materials.get_mut(&desktop.material) else {
        warn!("Material da tela 3D nao foi encontrado");
        return;
    };

    let same_size = images
        .get(&desktop.current_texture)
        .map(|image| {
            image.texture_descriptor.size.width == frame.width()
                && image.texture_descriptor.size.height == frame.height()
        })
        .unwrap_or(false);

    if same_size {
        // Alterar o asset existente evita recriar textura, binding e handle a 60 FPS.
        if let Some(image) = images.get_mut(&desktop.current_texture) {
            image.data.copy_from_slice(frame.data());
        }
    } else {
        let new_texture = images.add(new_desktop_image(
            frame.width(),
            frame.height(),
            frame.data().to_vec(),
        ));
        material.base_color_texture = Some(new_texture.clone());

        if let Some(stale) = desktop.stale_texture.take() {
            images.remove(stale.id());
        }
        let previous = std::mem::replace(&mut desktop.current_texture, new_texture);
        desktop.stale_texture = Some(previous);
    }

    if !stats.first_frame_uploaded {
        info!(
            width = frame.width(),
            height = frame.height(),
            bytes = frame.data().len(),
            signature,
            "Primeiro frame ligado ao material 3D"
        );
        stats.first_frame_uploaded = true;
    }

    stats.frames = stats.frames.saturating_add(1);
    if frame_is_black {
        stats.black_frames = stats.black_frames.saturating_add(1);
    }

    if stats.last_report.elapsed() >= Duration::from_secs(1) {
        let elapsed = stats.last_report.elapsed().as_secs_f64();
        let fps = f64::from(stats.frames) / elapsed;
        info!(
            fps = format_args!("{fps:.1}"),
            frames = stats.frames,
            interval_ms = (elapsed * 1_000.0).round() as u64,
            black_frames = stats.black_frames,
            "FPS enviado ao ambiente SBS"
        );

        if stats.frames > 0 && stats.black_frames == stats.frames {
            warn!(
                "Todos os frames recebidos estao pretos; selecione uma janela diferente do VibesVR"
            );
        }

        stats.frames = 0;
        stats.black_frames = 0;
        stats.last_report = Instant::now();
    }
}

fn sampled_rgb_signature(data: &[u8]) -> u64 {
    data.chunks_exact(4)
        .step_by(4096)
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, pixel| {
            pixel[..3].iter().fold(hash, |value, byte| {
                (value ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
            })
        })
}

fn looks_black(data: &[u8]) -> bool {
    // Amostra pixels espalhados pelo frame para nao percorrer 8 MiB a cada
    // quadro. O quarto byte e o alpha e nao entra no teste.
    data.chunks_exact(4)
        .step_by(4096)
        .all(|pixel| pixel[0] <= 4 && pixel[1] <= 4 && pixel[2] <= 4)
}

fn test_pattern_rgba(width: u32, height: u32) -> Vec<u8> {
    let mut data = vec![0; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let tile_is_blue = ((x / 96) + (y / 96)) % 2 == 0;
            let color = if tile_is_blue {
                [20, 85, 210, 255]
            } else {
                [8, 18, 42, 255]
            };

            let offset = ((y * width + x) * 4) as usize;
            data[offset..offset + 4].copy_from_slice(&color);
        }
    }

    data
}

fn new_desktop_image(width: u32, height: u32, data: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    // A textura recebe novos pixels durante a execucao e tambem e amostrada
    // pelo StandardMaterial da tela do cinema.
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image
}

fn update_eye_viewports(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&Eye, &mut Camera)>,
    mut resized: EventReader<WindowResized>,
    mut initialized: Local<bool>,
) {
    let must_update = !*initialized || resized.read().next().is_some();
    if !must_update {
        return;
    }

    let Ok(window) = windows.get_single() else {
        return;
    };

    let width = window.physical_width();
    let height = window.physical_height();
    if width < 2 || height == 0 {
        return;
    }

    let left_width = width / 2;
    let right_width = width - left_width;

    for (eye, mut camera) in &mut cameras {
        let (x, eye_width) = match eye {
            Eye::Left => (0, left_width),
            Eye::Right => (left_width, right_width),
        };

        camera.viewport = Some(Viewport {
            physical_position: UVec2::new(x, 0),
            physical_size: UVec2::new(eye_width, height),
            ..default()
        });
    }

    *initialized = true;
    info!(width, height, "Viewports SBS atualizados");
}

fn keyboard_controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut exit: EventWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.send(AppExit::Success);
    }

    if keys.just_pressed(KeyCode::F11) {
        let Ok(mut window) = windows.get_single_mut() else {
            return;
        };

        window.mode = match window.mode {
            WindowMode::Windowed => WindowMode::BorderlessFullscreen,
            _ => WindowMode::Windowed,
        };

        if window.mode != WindowMode::Windowed {
            warn!(
                "Fullscreen no mesmo monitor capturado pode causar espelho infinito ou tela preta"
            );
        }
    }
}
