import 'package:copypaste_flutter/features/modules/models/module_models.dart';
import 'package:copypaste_flutter/shared/forms/dynamic_form_draft.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test(
    'selects one compatible model and validates multiple language choices',
    () async {
      const languages = DynamicFormField(
        id: 'languages',
        title: 'Search languages',
        kind: DynamicFormFieldKind.choices,
        defaultValue: <String>[],
        required: true,
        options: [
          DynamicFormChoice(id: 'en', title: 'English'),
          DynamicFormChoice(id: 'uk', title: 'Ukrainian'),
        ],
      );
      const module = InstalledModule(
        id: 'search',
        title: 'Search',
        description: '',
        version: '1.0.0',
        enabled: true,
        sizeBytes: 0,
        commands: [],
        preferenceFields: [languages],
        preferences: {},
        searchLanguageField: 'languages',
        searchModels: [
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
      final draft = DynamicFormDraft(
        fields: [languages],
        initial: {},
        accepts: module.acceptsPreferences,
      );
      expect(draft.valid, isFalse);
      draft.setValue('languages', ['en']);
      expect(draft.valid, isTrue);
      expect(module.searchModelFor(draft.values)?.id, 'english');
      draft.setValue('languages', ['en', 'uk']);
      expect(draft.valid, isTrue);
      expect(module.searchModelFor(draft.values)?.id, 'multi');
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
      const image = DynamicFormField(
        id: 'image_path',
        title: 'Image',
        kind: DynamicFormFieldKind.file,
        defaultValue: '',
        required: true,
        acceptedExtensions: ['png'],
      );
      final picker = _Picker();
      final draft = DynamicFormDraft(
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

class _Picker implements DynamicFormInputPicker {
  int disposed = 0;
  @override
  Future<SelectedDynamicFormInput?> chooseInput(DynamicFormField field) async =>
      SelectedDynamicFormInput(
        path: '/private/image.png',
        name: 'image.png',
        dispose: () async {
          disposed++;
        },
      );
}
