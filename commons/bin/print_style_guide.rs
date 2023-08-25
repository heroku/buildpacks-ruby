use commons::fun_run::CommandWithName;
use commons::output::fmt;
use commons::output::interface::Logger;
use commons::output::log::BuildLog;
use indoc::formatdoc;
use std::io::stdout;
use std::process::Command;
use ascii_table::AsciiTable;

fn main() {
    println!(
        "{}",
        formatdoc! {"

            Living build output style guide
            ===============================
        "}
    );

    {
        let help_prefix = fmt::help_prefix();
        let mut log = BuildLog::new(stdout()).buildpack_name("Section logging features");
        log = log
            .section("Section heading example")
            .step_and("step example")
            .step_and("step example two")
            .end_section();

        log = log
            .section("Section and step description")
            .step_and(
                "A section should be a noun i.e. 'Ruby Version', consider this the section topic.",
            )
            .step_and("A step should be a verb i.e. 'Downloading'")
            .step_and("Related verbs should be nested under a single section")
            .step_and(&formatdoc! {"
                Steps can be multiple lines long
                However they're best as short, factual,
                descriptions of what the program is doing.
            "}.trim())
            .step_and("Prefer a single line when possible")
            .step_and("Sections and steps are sentence cased with no ending puncuation")
            .step_and(&format!("{help_prefix} capitalize the first letter"))
            .end_section();

        let mut command = Command::new("bash");
        command.args(["-c", "ps aux | grep cargo"]);

        let mut stream = log.section("Timer steps")
        .step_and("Long running code should execute with a timer printing to the UI, to indicate the progam did not hang.")
        .step_and("Example:")
        .step_timed("Background progress timer")
        .finish_timed_step()
        .step_and("Output can be streamed. Mostly from commands. Example:")
        .step_timed_stream(&format!("Running {}", fmt::command(command.name())));

        command.stream_output(stream.io(), stream.io()).unwrap();
        log = stream.finish_timed_stream().end_section();
        drop(log);
    }


    {
        let debug_info = fmt::debug_info_prefix();
        let cmd_error = Command::new("iDoNotExist").named_output().err().unwrap();

        let mut log = BuildLog::new(stdout()).buildpack_name("Error and warnings");
        log = log
            .section("Debug information")
            .step_and("Should go above errors in section/step format")
            .end_section();

        log = log
            .section(&debug_info)
            .step_and(&cmd_error.to_string())
            .end_section();

        log.error(&formatdoc! {"
            Error: This is an error header

            This is the error body. Use an error for when the build cannot continue.
            An error should include a header with a short description of why it cannot continue.

            The body should include what error state was observed, why that's a problem, and
            what remediation steps an application owner using the buildpack to deploy can
            take to solve the issue.
        "});

        log.warning(&formatdoc! {"
            Warning: This is a warning header

            Warnings are for when we know for a fact a problem exists
            but it's not bad enough to abort the build.
        "});
        log.important(&formatdoc! {"
            Important: This is important

            Important is for when there's critical information that needs to be read
            however it may or may not be a problem. If we know for a fact that there's
            a problem then use a warning instead.

            An example of something that is important but might not be a problem is
            that an application owner upgraded to a new stack.
        "});
    }

    {
        let mut log = BuildLog::new(stdout()).buildpack_name("Formatting helpers");

        log = log
            .section("Description of this section")
            .step_and(&formatdoc! {"
                Formatting helpers can be used to enhance log output:
            "})
            .end_section();

        let mut table = AsciiTable::default();
        table.set_max_width(240);
        table.column(0).set_header("Example");
        table.column(1).set_header("Code");
        table.column(2).set_header("When to use");


        let mut data: Vec<Vec<String>> = Vec::new();
        data.push(vec![fmt::value("2.3.4"), "fmt::value(\"2.3.f\")".to_string(), "With versions, file names or other important values worth highlighting".to_string()]);
        data.push(vec![fmt::url("https://www.schneems.com"), "fmt::url(\"https://www.schneems.com\")".to_string(), "With urls".to_string()]);
        data.push(vec![fmt::command("bundle install"), "fmt::command(command.name())".to_string(), "With commands (alongside of `fun_run::CommandWithName`)".to_string()]);
        data.push(vec![fmt::details("extra information"), "fmt::details(\"extra information\")".to_string(), "Add specific information at the end of a line i.e. 'Cache cleared (ruby version changed)'".to_string()]);
        table.print(data);
        drop(log);
    }
}

