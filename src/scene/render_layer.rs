use wgpu::RenderPass;

pub trait Layer<'a> {
    fn sub_render_pass<'b>(&'a self, render_pass: &'b mut wgpu::RenderPass<'a>);
}

pub struct BasicLayer<B> {
    pub pipeline: wgpu::RenderPipeline,
    pub buffer: B,
}

pub struct VertexOnly {
    vertex: wgpu::Buffer,
    vertex_num: usize,
}

pub struct VertexAndInstances {
    pub vertex: wgpu::Buffer,
    pub vertex_num: usize,
    pub instance: wgpu::Buffer,
    pub instance_num: usize,
}

pub struct VertexAndIndexes {
    pub vertex: wgpu::Buffer,
    // pub vertex_num: usize,
    pub index: wgpu::Buffer,
    pub index_num: usize,
}

impl<'a> Layer<'a> for BasicLayer<VertexOnly> {
    fn sub_render_pass<'b>(&'a self, render_pass: &'b mut RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.buffer.vertex.slice(..));
        render_pass.draw(0..(self.buffer.vertex_num as _), 0..1);
    }
}

impl<'a> Layer<'a> for BasicLayer<VertexAndIndexes> {
    fn sub_render_pass<'b>(&'a self, render_pass: &'b mut RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.buffer.vertex.slice(..));
        render_pass.set_index_buffer(self.buffer.index.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..(self.buffer.index_num as _), 0, 0..1);
    }
}

impl<'a> Layer<'a> for BasicLayer<VertexAndInstances> {
    fn sub_render_pass<'b>(&'a self, render_pass: &'b mut RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_vertex_buffer(0, self.buffer.vertex.slice(..));
        render_pass.set_vertex_buffer(1, self.buffer.instance.slice(..));
        render_pass.draw(
            0..(self.buffer.vertex_num as _),
            0..(self.buffer.instance_num as _),
        );
    }
}
