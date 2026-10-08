import 'package:flutter_test/flutter_test.dart';
import 'package:copypaste_flutter/features/modules/controller/module_form_draft.dart';
import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/features/modules/repository/modules_repository.dart';

void main() {
  test(
    'selects one compatible model and validates multiple language choices',
    () async {
      const languages = ModuleField(
        id: 'languages',
        title: 'Search languages',
        kind: ModuleFieldKind.choices,
        defaultValue: <String>[],
        required: true,
        options: [
          ModuleChoice(id: 'en', title: 'English'),
          ModuleChoice(id: 'uk', title: 'Ukrainian'),
        ],
      );
      final draft = ModuleFormDraft(
        fields: [languages],
        initial: {},
        languageField: 'languages',
        models: const [
          ModuleSearchModel(
            id: 'english',
            title: 'English',
            languages: ['en'],
            sizeBytes: 20,
            available: false,
          ),
          ModuleSearchModel(
            id: 'multi',
            title: 'Multilingual',
            languages: ['en', 'uk'],
            sizeBytes: 100,
            available: false,
          ),
        ],
      );
      expect(draft.valid, isFalse);
      draft.setValue('languages', ['en']);
      expect(draft.valid, isTrue);
      expect(draft.selectedModel?.id, 'english');
      draft.setValue('languages', ['en', 'uk']);
      expect(draft.valid, isTrue);
      expect(draft.selectedModel?.id, 'multi');
      draft.setValue('languages', ['en', 'en']);
      expect(draft.valid, isFalse);
      draft.setValue('languages', ['unknown']);
      expect(draft.valid, isFalse);
      await draft.close();
    },
  );

  test(
    'selected files remain available until the invocation draft closes',
    () async {
      const image = ModuleField(
        id: 'image_path',
        title: 'Image',
        kind: ModuleFieldKind.file,
        defaultValue: '',
        required: true,
        acceptedExtensions: ['png'],
      );
      final picker = _Picker();
      final draft = ModuleFormDraft(
        fields: [image],
        initial: {},
        picker: picker,
      );
      expect(draft.valid, isFalse);
      await draft.chooseFile(image);
      expect(draft.valid, isTrue);
      expect(draft.fileName('image_path'), 'image.png');
      expect(draft.values['image_path'], '/private/image.png');
      expect(picker.disposed, 0);
      await draft.close();
      expect(picker.disposed, 1);
      await draft.close();
      expect(picker.disposed, 1);
    },
  );
}

class _Picker implements ModuleInputPicker {
  int disposed = 0;
  @override
  Future<SelectedModuleInput?> chooseInput(ModuleField field) async =>
      SelectedModuleInput(
        path: '/private/image.png',
        name: 'image.png',
        dispose: () async {
          disposed++;
        },
      );
}
