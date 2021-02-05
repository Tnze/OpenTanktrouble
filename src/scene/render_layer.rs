trait Layer {
    fn sub_render_pass(&self, render_pass: &mut wgpu::RenderPass);
}

fn default_render_pipeline(
    label: &str,
    vert_shader: wgpu::ShaderModule,
    frag_shader: wgpu::ShaderModule,
) {}
