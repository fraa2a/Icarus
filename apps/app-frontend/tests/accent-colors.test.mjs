import assert from 'node:assert/strict'
import test from 'node:test'
import { ACCENT_COLORS, contrastRatio, recolorPixels, validAccent } from '../src/utils/accent-colors.mjs'

test('every accent button meets WCAG AA in both themes', () => {
	for (const palette of Object.values(ACCENT_COLORS)) {
		assert.ok(contrastRatio(palette.light, '#ffffff') >= 4.5)
		assert.ok(contrastRatio(palette.dark, '#000000') >= 4.5)
	}
})
test('unknown stored accents safely fall back to purple', () => {
	assert.equal(validAccent('unknown'), 'purple')
	assert.equal(validAccent(null), 'purple')
	assert.equal(validAccent('green'), 'green')
})
test('recoloring preserves transparency and neutral detail while tinting wings and backdrop', () => {
	const pixels = new Uint8ClampedArray([244, 233, 149, 255, 129, 12, 194, 128, 20, 20, 20, 255, 80, 40, 10, 0])
	const output = recolorPixels(pixels, '#3b82f6')
	assert.deepEqual(Array.from(output.slice(8)), [20, 20, 20, 255, 80, 40, 10, 0])
	assert.equal(output[7], 128)
	assert.ok(output[2] > output[0])
	assert.ok(output[6] > output[4])
})
