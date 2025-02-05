import * as wasm from './pkg';
import * as copy from 'copy-to-clipboard';
import notie from 'notie';
import { editor as monacoEditor } from 'monaco-editor/esm/vs/editor/editor.api'
import { output } from './webpack.config';

require('notie/dist/notie.min.css');

const samplePayload = `[
	{
		"add": [
			"A"
		],
		"message": "RegisterNetworks",
		"remove": []
	}
]
`;

// https://github.com/microsoft/monaco-editor/issues/2874
self.MonacoEnvironment = {
	getWorkerUrl: function (moduleId, label) {
		return './json.worker.bundle.js';
	}
};

var editor = monacoEditor.create(document.getElementById('container'), {
	value: samplePayload,
	language: 'json',
	minimap: {
		enabled: false
	},
	theme: 'vs-light'
});

document.getElementById('compile-button').onclick = function () {
	let input = editor.getValue();

	try {
		let outputType = (<HTMLSelectElement>document.getElementById('output-type')).value;
		let isCalldata = outputType === 'calldata';
		console.log(`Output type is ${outputType}`);
		let compiled = wasm.compile(input, isCalldata);
		(<HTMLInputElement>document.getElementById('compiled')).value = toHexString(compiled);
	}
	catch (e: any) {
		notie.alert({ text: (<string>e), time: 2, type: 'error' });
	}
};

document.getElementById('copy-to-clipboard').onclick = function () {
	let compiled = (<HTMLInputElement>document.getElementById('compiled')).value;
	notie.alert({ text: `Copied ${compiled.length} characters to the clipboard.`, time: 1, type: 'success' });
	copy(compiled);
};

document.getElementById('clear-all').onclick = function () {
	editor.setValue('');
	(<HTMLFormElement>document.getElementById("form")).reset();
}

document.getElementById('verify-compiled').oninput = function () {
	let compiled = (<HTMLInputElement>document.getElementById('compiled')).value;
	let expected = (<HTMLInputElement>document.getElementById('verify-compiled')).value;

	let text;
	if (compiled === expected || `0x${compiled}` === expected) {
		text = '✓ matches'
	} else {
		text = '✗ does not match'
	}

	(<HTMLParagraphElement>document.getElementById('verify-result')).innerText = text;
}


function toHexString(byteArray: Uint8Array): string {
	var s = '';
	byteArray.forEach(function (byte) {
		s += ('0' + (byte & 0xFF).toString(16)).slice(-2);
	});
	return s;
}