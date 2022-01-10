import 'dart:typed_data';
import 'dart:convert';
import 'dart:io';
import 'package:ic_tools/ic_tools.dart';
import 'package:ic_tools/common.dart' as common;
import 'package:ic_tools/candid.dart';
import 'package:ic_tools/tools.dart';

import 'package:crypto/crypto.dart';




Future<void> main() async {
    // await create_controller();
    // await create_canister();
    // await canister_status();
    // await top_up_canister();
    // await canister_status();
    // await put_code_on_the_canister();
    // await canister_status();
    // await call_canister();
    await put_frontcode_build_web();
    await call_canister();



}


Future<void> create_controller() async {
    Caller controller = CallerEd25519.new_keys();
    print(controller);
    print(common.principal_as_an_IcpCountId(controller.principal));
    print('pub: ${controller.public_key}');
    print('priv: ${controller.private_key}');
    
}


Caller controller = CallerEd25519(
    public_key: Uint8List.fromList([74, 155, 194, 191, 169, 170, 197, 30, 94, 189, 77, 230, 39, 116, 53, 17, 24, 38, 237, 38, 5, 198, 49, 141, 25, 22, 19, 234, 239, 169, 13, 37]),
    private_key: Uint8List.fromList([30, 59, 248, 28, 231, 2, 33, 227, 106, 121, 113, 195, 42, 30, 155, 8, 159, 189, 171, 119, 249, 30, 14, 198, 18, 172, 97, 142, 143, 231, 86, 240]),
);

// Caller controller = CallerEd25519(
//     public_key: Uint8List.fromList([204, 111, 49, 65, 97, 240, 72, 38, 197, 169, 218, 47, 135, 138, 5, 112, 189, 95, 18, 223, 161, 175, 106, 167, 201, 190, 156, 91, 242, 27, 237, 160]),
//     private_key: Uint8List.fromList([231, 127, 35, 33, 67, 179, 1, 154, 134, 71, 61, 73, 88, 205, 145, 212, 175, 103, 111, 69, 64, 227, 219, 60, 226, 69, 151, 223, 36, 128, 75, 108]),
// );

Canister canister = Canister(Principal('thp4z-laaaa-aaaam-qaaea-cai'));
// Canister canister = Canister(Principal('bayhi-7yaaa-aaaai-qahca-cai'));


Future<void> create_canister() async {
    print(await common.check_icp_balance(common.principal_as_an_IcpCountId(controller.principal)));
    Principal can_id = await common.create_canister(controller, 0.2);
    print(can_id);
    Canister can = Canister(can_id);
    print(await can.controllers());
    print(await common.check_icp_balance(common.principal_as_an_IcpCountId(controller.principal)));
}

Future<void> canister_status() async {
    print(await common.check_canister_status(controller, canister.principal));

}

Future<void> top_up_canister() async {
    print(await common.check_icp_balance(common.principal_as_an_IcpCountId(controller.principal)));
    await common.top_up_canister(controller, 0.01, canister.principal);
    print(await common.check_icp_balance(common.principal_as_an_IcpCountId(controller.principal)));

}

Future<void> put_code_on_the_canister() async {
    // Uint8List install_code_arg = c_forwards([
    //     Nat64(58)
    // ]);
    await common.put_code_on_the_canister(
        controller,
        canister.principal,
        File('../cycles-transfer-station/target/wasm32-unknown-unknown/release/cycles_transfer_station-opt.wasm').readAsBytesSync(),
        'reinstall',
        // install_code_arg
    );
}



Future<void> call_canister() async {
    late List<CandidType> cs;

    // cs = c_backwards(await canister.call(
    //     calltype: 'call',
    //     method_name: 'public_stable_size',

    // ));
    // print(cs);


    // cs = c_backwards(await canister.call(
    //     calltype: 'call',
    //     method_name: 'public_stable_grow',
    //     put_bytes: c_forwards([
    //         Nat64(200),
    //     ])

    // ));
    // print(cs);

    // cs = c_backwards(await canister.call(
    //     calltype: 'call',
    //     method_name: 'public_stable_size',
    // ));
    // print(cs);


    // cs = c_backwards(await canister.call(
    //     calltype: 'call',
    //     method_name: 'public_stable_write',
    //     put_bytes: c_forwards([
    //         Nat64(2),
    //         Blob([1,2,3]),
    //     ])

    // ));
    // print(cs);

    cs = c_backwards(await canister.call(
        calltype: 'call',
        method_name: 'public_stable_size',
    ));
    print(cs);

    cs = c_backwards(await canister.call(
        calltype: 'call',
        method_name: 'public_stable_bytes',
    ));
    Blob b = cs[0] as Blob;
    // print(b.bytes);
    print(b.length);

    // cs = c_backwards(await canister.call(
    //     calltype: 'call',
    //     method_name: 'public_stable_read',
    //     put_bytes: c_forwards([
    //         Nat64(0),
    //         Nat64(20)
    //     ])

    // ));
    // print((cs[0] as Blob).bytes);

    // cs = c_backwards(await canister.call(
    //     calltype: 'call',
    //     method_name: 'public_clear_file_hashes',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);




    // cs = c_backwards(await canister.call(
    //     calltype: 'call',
    //     method_name: 'upload_frontcode_file_chunks',
    //     put_bytes: c_forwards([
    //         Text('/'),
    //         Record.oftheMap({
    //             'content_type': Text('text/plain; charset=utf-8'),
    //             'content_encoding': Text(''),
    //             'content': Blob(utf8.encode('Hello'))
    //         })
    //     ])
    // ));
    // print(cs);

    // cs = c_backwards(await canister.call(
    //     calltype: 'call',
    //     method_name: 'upload_frontcode_file_chunks',
    //     put_bytes: c_forwards([
    //         Text('/sample.html'),
    //         Record.oftheMap({
    //             'content_type': Text('text/html; charset=utf-8'),
    //             'content_encoding': Text('gzip'),
    //             'content': Blob(gzip.encode(utf8.encode('<html><body><h1>Levi is Awesome.</h1></body></html>')))
    //         })
    //     ])
    // ));
    // print(cs);

    cs = c_backwards(await canister.call(
        calltype: 'call',
        method_name: 'public_get_file_hashes',
    ));
    // print(cs);
    for (Record r in (cs[0] as Vector).cast_vector<Record>()) {
        print(r[0]);
    }


    // cs = c_backwards(await canister.call(
    //     calltype: 'call',
    //     method_name: 'upload_frontcode_file_chunks',
    //     put_bytes: c_forwards([
    //         Text('/hello'),
    //         Blob(utf8.encode('hello'))
    //     ])
    // ));
    // print(cs);

    // cs = c_backwards(await canister.call(
    //     calltype: 'call',
    //     method_name: 'public_get_file_hashes',
    //     put_bytes: c_forwards([])
    // ));
    // print(cs);


    
}



Future<void> put_frontcode_build_web() async {
    Directory build_web_dir = Directory('/home/coder/Documents/code/cycles-transfer-station/front/build/web/');
    await for (FileSystemEntity fse in build_web_dir.list(recursive: true, followLinks: false)) {
        print(fse.path);
        if (await FileSystemEntity.isFile(fse.path)) {
            List<CandidType> cs = c_backwards(await canister.call(
                calltype: 'call',
                method_name: 'upload_frontcode_file_chunks',
                put_bytes: c_forwards([
                    Text( fse.path.contains('/index.html') ? '/' : fse.path.replaceFirst('/home/coder/Documents/code/cycles-transfer-station/front/build/web', '')),
                    Record.oftheMap({
                        'content_type': Text(''),
                        'content_encoding': Text('gzip'),
                        'content': Blob(gzip.encode(File(fse.path).readAsBytesSync()))
                    })
                ])
            ));
            print(cs);


        }
    }
}