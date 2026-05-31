// Minimal Ghostty-compatible cursor shader probe.
// The shader intentionally reads the cursor uniforms Yazelix needs before
// doing anything visually ambitious.
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec4 source = texture(iChannel0, fragCoord / iResolution.xy);
    vec2 cursor_min = iCurrentCursor.xy;
    vec2 cursor_max = iCurrentCursor.xy + iCurrentCursor.zw;
    bool inside_cursor =
        fragCoord.x >= cursor_min.x &&
        fragCoord.x <= cursor_max.x &&
        fragCoord.y >= cursor_min.y &&
        fragCoord.y <= cursor_max.y;

    if (inside_cursor && iCursorVisible == 1) {
        fragColor = mix(source, iCurrentCursorColor, 0.50);
    } else {
        fragColor = source;
    }
}
