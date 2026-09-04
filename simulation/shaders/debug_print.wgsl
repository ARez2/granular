const kCharBlank: f32 = 12.0;
const kCharMinus: f32 = 11.0;
const kCharDecimalPoint: f32 = 10.0;

fn floatMod(x: f32, y: f32) -> f32 {
    return x - y * floor(x / y);
}

fn InRect(vUV: vec2<f32>, vRect: vec4<f32>) -> f32 {
    let vTestMin = step(vRect.xy, vUV);
    let vTestMax = step(vUV, vRect.zw);
    let vTest = vTestMin * vTestMax;
    return vTest.x * vTest.y;
}

fn SampleDigit(fDigit: f32, vUV: vec2<f32>) -> f32 {
    let x0 = 0.0 / 4.0;
    let x1 = 1.0 / 4.0;
    let x2 = 2.0 / 4.0;
    let x3 = 3.0 / 4.0;
    let x4 = 4.0 / 4.0;

    let y0 = 0.0 / 5.0;
    let y1 = 1.0 / 5.0;
    let y2 = 2.0 / 5.0;
    let y3 = 3.0 / 5.0;
    let y4 = 4.0 / 5.0;
    let y5 = 5.0 / 5.0;

    var vRect0 = vec4<f32>(0.0);
    var vRect1 = vec4<f32>(0.0);
    var vRect2 = vec4<f32>(0.0);

    if (fDigit < 0.5) {
        vRect0 = vec4(x0, y0, x3, y5);
        vRect1 = vec4(x1, y1, x2, y4);
    } else if (fDigit < 1.5) {
        vRect0 = vec4(x1, y0, x2, y5);
        vRect1 = vec4(x0, y0, x0, y0);
    } else if (fDigit < 2.5) {
        vRect0 = vec4(x0, y0, x3, y5);
        vRect1 = vec4(x0, y3, x2, y4);
        vRect2 = vec4(x1, y1, x3, y2);
    } else if (fDigit < 3.5) {
        vRect0 = vec4(x0, y0, x3, y5);
        vRect1 = vec4(x0, y3, x2, y4);
        vRect2 = vec4(x0, y1, x2, y2);
    } else if (fDigit < 4.5) {
        vRect0 = vec4(x0, y1, x2, y5);
        vRect1 = vec4(x1, y2, x2, y5);
        vRect2 = vec4(x2, y0, x3, y3);
    } else if (fDigit < 5.5) {
        vRect0 = vec4(x0, y0, x3, y5);
        vRect1 = vec4(x1, y3, x3, y4);
        vRect2 = vec4(x0, y1, x2, y2);
    } else if (fDigit < 6.5) {
        vRect0 = vec4(x0, y0, x3, y5);
        vRect1 = vec4(x1, y3, x3, y4);
        vRect2 = vec4(x1, y1, x2, y2);
    } else if (fDigit < 7.5) {
        vRect0 = vec4(x0, y0, x3, y5);
        vRect1 = vec4(x0, y0, x2, y4);
    } else if (fDigit < 8.5) {
        vRect0 = vec4(x0, y0, x3, y5);
        vRect1 = vec4(x1, y1, x2, y2);
        vRect2 = vec4(x1, y3, x2, y4);
    } else if (fDigit < 9.5) {
        vRect0 = vec4(x0, y0, x3, y5);
        vRect1 = vec4(x1, y3, x2, y4);
        vRect2 = vec4(x0, y1, x2, y2);
    } else if (fDigit < 10.5) {
        vRect0 = vec4(x1, y0, x2, y1);
    } else if (fDigit < 11.5) {
        vRect0 = vec4(x0, y2, x3, y3);
    }

    let fResult =
        InRect(vUV, vRect0) +
        InRect(vUV, vRect1) +
        InRect(vUV, vRect2);

    return floatMod(fResult, 2.0);
}

fn PrintValue(
    vStringCharCoords: vec2<f32>,
    fValue: f32,
    fMaxDigits: f32,
    fDecimalPlaces: f32,
) -> f32 {

    let fAbsValue = abs(fValue);
    let fStringCharIndex = floor(vStringCharCoords.x);
    let fLog10Value = log2(fAbsValue) / log2(10.0);
    let fBiggestDigitIndex = max(floor(fLog10Value), 0.0);
    var fDigitCharacter = kCharBlank;
    var fDigitIndex = fMaxDigits - fStringCharIndex;
    if (fDigitIndex > (-fDecimalPlaces - 1.5)) {
        if (fDigitIndex > fBiggestDigitIndex) {
            if (fValue < 0.0) {
                if (fDigitIndex < (fBiggestDigitIndex + 1.5)) {
                    fDigitCharacter = kCharMinus;
                }
            }
        } else {
            if (fDigitIndex == -1.0) {
                if (fDecimalPlaces > 0.0) {
                    fDigitCharacter = kCharDecimalPoint;
                }
            } else {
                if (fDigitIndex < 0.0) {
                    fDigitIndex += 1.0;
                }
                let fDigitValue =
                    fAbsValue / pow(10.0, fDigitIndex);
                fDigitCharacter =
                    floatMod(floor(0.0001 + fDigitValue), 10.0);
            }
        }
    }

    let vCharPos = vec2<f32>(
        fract(vStringCharCoords.x),
        1.0 - vStringCharCoords.y,
    );

    return SampleDigit(fDigitCharacter, vCharPos);
}


fn digits_before_decimal(x: f32) -> i32 {
    var v = abs(x);
    var digits = 1;

    while (v >= 10.0) {
        v /= 10.0;
        digits++;
    }

    if (abs(x) < 1.0) {
        return 0;
    }

    return digits;
}


