//! A small Direct3D window used only to validate the external FPS collector.
use windows::{
    core::w,
    Win32::{
        Foundation::*,
        Graphics::{Direct3D::*, Direct3D11::*, Dxgi::Common::*, Dxgi::*},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::*,
    },
};
pub fn run() -> windows::core::Result<()> {
    unsafe {
        unsafe extern "system" fn procedure(
            window: HWND,
            message: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            DefWindowProcW(window, message, wparam, lparam)
        }
        let instance = GetModuleHandleW(None)?;
        let class = w!("SWAPPRISM_FPS_PROBE");
        let wnd = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wnd);
        let window = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("Swap Prism - prueba Direct3D"),
            WS_OVERLAPPEDWINDOW,
            40,
            40,
            460,
            180,
            None,
            None,
            instance,
            None,
        )?;
        let desc = DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC {
                Width: 460,
                Height: 180,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ..Default::default()
            },
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            OutputWindow: window,
            Windowed: BOOL(1),
            SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
            ..Default::default()
        };
        let mut swap = None;
        let mut device = None;
        let mut context = None;
        D3D11CreateDeviceAndSwapChain(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            None,
            D3D11_CREATE_DEVICE_FLAG(0),
            None,
            D3D11_SDK_VERSION,
            Some(&desc),
            Some(&mut swap),
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
        let swap = swap.unwrap();
        let device = device.unwrap();
        let context = context.unwrap();
        let buffer: ID3D11Texture2D = swap.GetBuffer(0)?;
        let mut target = None;
        device.CreateRenderTargetView(&buffer, None, Some(&mut target))?;
        let target = target.unwrap();
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        let start = std::time::Instant::now();
        let mut msg = MSG::default();
        while start.elapsed() < std::time::Duration::from_secs(10) {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let phase = (start.elapsed().as_secs_f32() * 2.).sin() * 0.1 + 0.2;
            context.ClearRenderTargetView(&target, &[0.05, phase, 0.2, 1.]);
            swap.Present(1, DXGI_PRESENT(0)).ok()?;
        }
        DestroyWindow(window)?;
        Ok(())
    }
}
