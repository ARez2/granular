#version 440


void main() {
    float x = float(int(gl_VertexIndex) - 1);
    float y = float(int(gl_VertexIndex & uint(1)) * 2 - 1);
    gl_Position = vec4(x, y, 0.0, 1.0);
}